use super::backend_view::{find_repo_root, parse_backend_view_value, reject_forbidden_output_keys};
use crate::schema::LocalGameView;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fmt::Debug;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const GAME_MODE: &str = "4p-red-single";

#[derive(Debug)]
pub struct BackendLocalSession {
    backend_game_id: Option<String>,
    latest_view: Option<LocalGameView>,
    terminal: bool,
    end_reason: Option<String>,
    next_request_id: u64,
    transport: Box<dyn BackendSessionTransport>,
}

impl BackendLocalSession {
    pub fn start(seed: u64) -> Result<Self, String> {
        let transport = Box::new(ProcessBackendSessionTransport::spawn()?);
        Self::start_with_transport(seed, transport)
    }

    pub fn start_with_transport(
        seed: u64,
        transport: Box<dyn BackendSessionTransport>,
    ) -> Result<Self, String> {
        let mut session = Self {
            backend_game_id: None,
            latest_view: None,
            terminal: false,
            end_reason: None,
            next_request_id: 1,
            transport,
        };
        let request_id = session.next_request_id();
        let response = session.transport.send_request(json!({
            "requestId": request_id,
            "type": "new",
            "gameMode": GAME_MODE,
            "seed": seed,
        }))?;
        session.apply_response(response)?;
        Ok(session)
    }

    pub fn get_view(&mut self) -> Result<LocalGameView, String> {
        if self.terminal {
            return self
                .latest_view
                .clone()
                .ok_or_else(|| "backend session ended without a view".into());
        }
        let request_id = self.next_request_id();
        let response = self
            .transport
            .send_request(json!({"requestId": request_id, "type": "get_view"}))?;
        self.apply_response(response)?;
        self.latest_view
            .clone()
            .ok_or_else(|| "backend get_view did not return a view".into())
    }

    pub fn latest_view(&self) -> Result<LocalGameView, String> {
        self.latest_view
            .clone()
            .ok_or_else(|| "backend session has no view".into())
    }

    pub fn submit_action(&mut self, action_id: u32) -> Result<LocalGameView, String> {
        if self.terminal {
            return Err(format!(
                "backend local session already ended: {}",
                self.end_reason.as_deref().unwrap_or("terminal")
            ));
        }
        let request_id = self.next_request_id();
        let response = self.transport.send_request(json!({
            "requestId": request_id,
            "type": "submit",
            "actionId": action_id,
        }))?;
        self.apply_response(response)?;
        self.latest_view
            .clone()
            .ok_or_else(|| "backend submit did not return a view".into())
    }

    pub fn close(&mut self) {
        let request_id = self.next_request_id();
        let _ = self
            .transport
            .send_request(json!({"requestId": request_id, "type": "close"}));
        self.terminal = true;
        self.end_reason = Some("closed".into());
    }

    fn next_request_id(&mut self) -> String {
        let request_id = format!("tauri-{}", self.next_request_id);
        self.next_request_id += 1;
        request_id
    }

    fn apply_response(&mut self, raw: Value) -> Result<(), String> {
        reject_forbidden_output_keys(&raw)?;
        let response: BackendLocalSessionResponse = serde_json::from_value(raw)
            .map_err(|error| format!("backend local-session response parse failed: {error}"))?;
        response.validate()?;

        if !response.ok {
            return Err(response
                .error
                .unwrap_or_else(|| "backend local-session request failed".into()));
        }
        self.validate_game_id(response.game_id.as_deref())?;

        if response.terminal {
            self.terminal = true;
            self.end_reason = response.end_reason;
            if let Some(view) = response.view {
                self.latest_view = Some(parse_backend_view_value(view)?);
            }
            self.mark_latest_view_terminal();
            if self.latest_view.is_none() {
                return Err("backend terminal response has no previous view".into());
            }
            return Ok(());
        }

        let view = response
            .view
            .ok_or_else(|| "backend active response is missing view".to_string())
            .and_then(parse_backend_view_value)?;
        self.latest_view = Some(view);
        self.terminal = false;
        self.end_reason = None;
        Ok(())
    }

    fn validate_game_id(&mut self, game_id: Option<&str>) -> Result<(), String> {
        let Some(game_id) = game_id else {
            return Ok(());
        };
        if let Some(existing) = &self.backend_game_id {
            if existing != game_id {
                return Err(format!(
                    "backend local-session gameId changed from {existing} to {game_id}"
                ));
            }
        } else {
            self.backend_game_id = Some(game_id.to_string());
        }
        Ok(())
    }

    fn mark_latest_view_terminal(&mut self) {
        let Some(view) = &mut self.latest_view else {
            return;
        };
        view.phase_label = "Terminal".into();
        view.engine.status = "terminal".into();
        view.notice = format!(
            "{} Backend local-session ended: {}.",
            view.notice,
            self.end_reason.as_deref().unwrap_or("terminal")
        );
        for action in &mut view.actions {
            action.enabled = false;
            action.hint = "Backend local-session already ended.".into();
        }
    }
}

impl Drop for BackendLocalSession {
    fn drop(&mut self) {
        if !self.terminal {
            self.close();
        }
    }
}

pub trait BackendSessionTransport: Debug + Send {
    fn send_request(&mut self, request: Value) -> Result<Value, String>;
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackendLocalSessionResponse {
    #[serde(rename = "type")]
    response_type: String,
    request_id: Option<String>,
    ok: bool,
    game_id: Option<String>,
    view: Option<Value>,
    terminal: bool,
    end_reason: Option<String>,
    error: Option<String>,
}

impl BackendLocalSessionResponse {
    fn validate(&self) -> Result<(), String> {
        if self.response_type != "local_session_response" {
            return Err(format!(
                "unexpected backend local-session response type: {}",
                self.response_type
            ));
        }
        let _ = &self.request_id;
        Ok(())
    }
}

#[derive(Debug)]
struct ProcessBackendSessionTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl ProcessBackendSessionTransport {
    fn spawn() -> Result<Self, String> {
        let repo_root = find_repo_root().ok_or_else(|| {
            "could not locate repository root with backend/pyproject.toml".to_string()
        })?;
        let mut child = Command::new("uv")
            .args([
                "run",
                "--project",
                "backend",
                "riichi-ai-trainer",
                "local-session",
            ])
            .current_dir(repo_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to start backend local-session: {error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "backend local-session stdin is unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "backend local-session stdout is unavailable".to_string())?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }
}

impl BackendSessionTransport for ProcessBackendSessionTransport {
    fn send_request(&mut self, request: Value) -> Result<Value, String> {
        let line = serde_json::to_string(&request)
            .map_err(|error| format!("backend local-session request encode failed: {error}"))?;
        writeln!(self.stdin, "{line}")
            .and_then(|_| self.stdin.flush())
            .map_err(|error| format!("backend local-session request write failed: {error}"))?;

        let mut output = String::new();
        let bytes = self
            .stdout
            .read_line(&mut output)
            .map_err(|error| format!("backend local-session response read failed: {error}"))?;
        if bytes == 0 {
            return Err("backend local-session exited without a response".into());
        }
        serde_json::from_str(&output)
            .map_err(|error| format!("backend local-session response JSON parse failed: {error}"))
    }
}

impl Drop for ProcessBackendSessionTransport {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Debug)]
    struct FakeTransport {
        responses: VecDeque<Value>,
        requests: Vec<Value>,
    }

    impl FakeTransport {
        fn new(responses: Vec<Value>) -> Self {
            Self {
                responses: responses.into(),
                requests: Vec::new(),
            }
        }
    }

    impl BackendSessionTransport for FakeTransport {
        fn send_request(&mut self, request: Value) -> Result<Value, String> {
            self.requests.push(request);
            self.responses
                .pop_front()
                .ok_or_else(|| "no fake response queued".into())
        }
    }

    fn view(action_id: u32, tile: &str) -> Value {
        json!({
            "schemaVersion": 1,
            "source": "backend_mapper",
            "engine": {
                "schemaVersion": 1,
                "source": "backend_riichienv_mapper",
                "status": "active",
                "capabilities": ["discard", "tsumo", "ron", "chi", "pon", "kan", "riichi", "pass"],
                "note": "Backend mapper"
            },
            "phaseLabel": "Human decision",
            "notice": "Backend mapper view.",
            "round": {"roundLabel": "kyoku 1", "honba": 0, "kyotaku": 0, "remainingTiles": 0, "dealerSeat": 0},
            "players": [
                {"seat": 0, "relationLabel": "自家", "wind": "E", "score": 25000, "riverTiles": [], "melds": [], "statusTags": ["thinking"], "isDealer": true, "isSelf": true},
                {"seat": 1, "relationLabel": "下家", "wind": "S", "score": 25000, "riverTiles": [], "melds": [], "statusTags": [], "isDealer": false, "isSelf": false},
                {"seat": 2, "relationLabel": "对面", "wind": "W", "score": 25000, "riverTiles": [], "melds": [], "statusTags": [], "isDealer": false, "isSelf": false},
                {"seat": 3, "relationLabel": "上家", "wind": "N", "score": 25000, "riverTiles": [], "melds": [], "statusTags": [], "isDealer": false, "isSelf": false}
            ],
            "selfHandTiles": ["1m", "2m"],
            "doraIndicators": ["5m"],
            "actions": [{"id": action_id, "type": "discard", "label": format!("dahai {tile}"), "hint": "合法动作", "enabled": true, "tile": tile}],
            "recommendations": [{"rank": 1, "actionId": action_id, "tile": tile, "label": format!("dahai {tile}"), "source": "deterministic_baseline", "status": "recommended", "note": "first"}]
        })
    }

    fn ok_response(request_id: &str, game_id: &str, view: Value) -> Value {
        json!({
            "type": "local_session_response",
            "requestId": request_id,
            "ok": true,
            "gameId": game_id,
            "view": view,
            "terminal": false,
            "endReason": null,
            "error": null
        })
    }

    #[test]
    fn backend_session_new_get_view_and_submit_return_backend_views() {
        let transport = Box::new(FakeTransport::new(vec![
            ok_response("tauri-1", "backend-1", view(1, "1m")),
            ok_response("tauri-2", "backend-1", view(2, "2m")),
            ok_response("tauri-3", "backend-1", view(3, "3m")),
        ]));
        let mut session = BackendLocalSession::start_with_transport(1, transport).unwrap();

        let first = session.get_view().unwrap();
        let submitted = session.submit_action(first.actions[0].id).unwrap();

        assert_eq!(first.source, "backend_mapper");
        assert_eq!(first.actions[0].id, 2);
        assert_eq!(submitted.engine.source, "backend_riichienv_mapper");
        assert_eq!(submitted.actions[0].id, 3);
        assert!(submitted.actions[0].enabled);
    }

    #[test]
    fn backend_session_rejects_error_response() {
        let transport = Box::new(FakeTransport::new(vec![json!({
            "type": "local_session_response",
            "requestId": "tauri-1",
            "ok": false,
            "gameId": null,
            "view": null,
            "terminal": false,
            "endReason": null,
            "error": "backend unavailable"
        })]));

        let error = BackendLocalSession::start_with_transport(1, transport).unwrap_err();

        assert!(error.contains("backend unavailable"));
    }

    #[test]
    fn backend_session_rejects_hidden_information_response() {
        let transport = Box::new(FakeTransport::new(vec![ok_response(
            "tauri-1",
            "backend-1",
            json!({
                "schemaVersion": 1,
                "source": "backend_mapper",
                "hiddenTiles": []
            }),
        )]));

        let error = BackendLocalSession::start_with_transport(1, transport).unwrap_err();

        assert!(error.contains("hidden-information"));
    }

    #[test]
    fn backend_session_marks_terminal_with_latest_view() {
        let transport = Box::new(FakeTransport::new(vec![
            ok_response("tauri-1", "backend-1", view(1, "1m")),
            json!({
                "type": "local_session_response",
                "requestId": "tauri-2",
                "ok": true,
                "gameId": "backend-1",
                "view": null,
                "terminal": true,
                "endReason": "exhaustive_draw",
                "error": null
            }),
        ]));
        let mut session = BackendLocalSession::start_with_transport(1, transport).unwrap();

        let terminal = session.submit_action(1).unwrap();

        assert_eq!(terminal.engine.status, "terminal");
        assert!(terminal.notice.contains("exhaustive_draw"));
        assert!(terminal.actions.iter().all(|action| !action.enabled));
    }

    #[test]
    fn backend_session_submit_error_preserves_latest_view() {
        let transport = Box::new(FakeTransport::new(vec![
            ok_response("tauri-1", "backend-1", view(1, "1m")),
            json!({
                "type": "local_session_response",
                "requestId": "tauri-2",
                "ok": false,
                "gameId": "backend-1",
                "view": null,
                "terminal": false,
                "endReason": null,
                "error": "unknown legal action id"
            }),
        ]));
        let mut session = BackendLocalSession::start_with_transport(1, transport).unwrap();

        let error = session.submit_action(99).unwrap_err();
        let latest = session.latest_view().unwrap();

        assert!(error.contains("unknown legal action id"));
        assert_eq!(latest.actions[0].id, 1);
        assert_eq!(latest.engine.status, "active");
    }
}
