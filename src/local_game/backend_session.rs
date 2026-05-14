use super::backend_view::{find_repo_root, parse_backend_view_value, reject_forbidden_output_keys};
use crate::schema::{LocalGameView, LocalReviewKeyChoice, LocalReviewSummary};
use serde::Deserialize;
use serde_json::{json, Value};
use std::env;
use std::fmt::Debug;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const GAME_MODE: &str = "4p-red-single";
const AI_WORKER_CMD_ENV: &str = "RIICHI_AI_TRAINER_AI_WORKER_CMD";
const AI_WORKER_TIMEOUT_MS_ENV: &str = "RIICHI_AI_TRAINER_AI_WORKER_TIMEOUT_MS";
const COACH_WORKER_CMD_ENV: &str = "RIICHI_AI_TRAINER_COACH_WORKER_CMD";
const COACH_WORKER_TIMEOUT_MS_ENV: &str = "RIICHI_AI_TRAINER_COACH_WORKER_TIMEOUT_MS";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackendLocalSessionConfig {
    pub ai_worker_cmd: Option<String>,
    pub ai_worker_timeout_ms: Option<u32>,
    pub coach_worker_cmd: Option<String>,
    pub coach_worker_timeout_ms: Option<u32>,
}

impl From<crate::config::LocalGameConfig> for BackendLocalSessionConfig {
    fn from(config: crate::config::LocalGameConfig) -> Self {
        Self {
            ai_worker_cmd: Some(config.ai_worker_cmd),
            ai_worker_timeout_ms: config.ai_worker_timeout_ms,
            coach_worker_cmd: Some(config.coach_worker_cmd),
            coach_worker_timeout_ms: config.coach_worker_timeout_ms,
        }
    }
}

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
    pub fn start(seed: u64, config: BackendLocalSessionConfig) -> Result<Self, String> {
        let transport = Box::new(ProcessBackendSessionTransport::spawn(config)?);
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
            if let Some(review_report) = response.review_report {
                let summary = parse_backend_review_report(review_report)?;
                if let Some(view) = &mut self.latest_view {
                    view.review_summary = summary;
                }
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
    review_report: Option<Value>,
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

#[derive(Debug, Deserialize)]
struct BackendReviewReport {
    schema_version: u32,
    summary: BackendReviewSummaryCounts,
    #[serde(default)]
    top_decision_mismatches: Vec<BackendReviewDecision>,
    #[serde(default)]
    key_choices: Vec<BackendReviewDecision>,
    #[serde(default)]
    all_decisions: Vec<BackendReviewDecision>,
}

#[derive(Debug, Deserialize)]
struct BackendReviewSummaryCounts {
    decision_count: u32,
    matched_recommendation_count: u32,
    mismatch_count: u32,
    #[serde(default)]
    attention_count: u32,
    #[serde(default)]
    fallback_count: u32,
    #[serde(default)]
    unavailable_count: u32,
    #[serde(default)]
    not_ranked_count: u32,
}

#[derive(Debug, Deserialize)]
struct BackendReviewDecision {
    turn_index: Option<u32>,
    selected_action_id: Option<u32>,
    selected_action_label: Option<String>,
    recommended_action_id: Option<u32>,
    recommended_action_label: Option<String>,
    category: Option<String>,
    recommendation_reason: Option<String>,
}

fn parse_backend_review_report(raw: Value) -> Result<LocalReviewSummary, String> {
    let report: BackendReviewReport = serde_json::from_value(raw)
        .map_err(|error| format!("backend reviewReport parse failed: {error}"))?;
    if report.schema_version != 1 {
        return Err(format!(
            "unsupported backend reviewReport schema_version: {}",
            report.schema_version
        ));
    }

    let choices_source = if !report.key_choices.is_empty() {
        &report.key_choices
    } else if !report.top_decision_mismatches.is_empty() {
        &report.top_decision_mismatches
    } else {
        &report.all_decisions
    };
    let key_choices = choices_source
        .iter()
        .take(5)
        .map(review_key_choice)
        .collect();

    Ok(LocalReviewSummary {
        schema_version: 1,
        source: "backend_review_report".into(),
        total_decisions: report.summary.decision_count,
        top1_matches: report.summary.matched_recommendation_count,
        mismatch_count: report.summary.mismatch_count,
        attention_count: if report.summary.attention_count > 0 {
            report.summary.attention_count
        } else {
            report.summary.mismatch_count
                + report.summary.unavailable_count
                + report.summary.not_ranked_count
        },
        fallback_count: if report.summary.fallback_count > 0 {
            report.summary.fallback_count
        } else {
            report.summary.unavailable_count
        },
        unavailable_count: report.summary.unavailable_count,
        not_ranked_count: report.summary.not_ranked_count,
        key_choices,
        note: "Backend review summary from terminal local-session.".into(),
    })
}

fn review_key_choice(decision: &BackendReviewDecision) -> LocalReviewKeyChoice {
    LocalReviewKeyChoice {
        turn_index: decision.turn_index.unwrap_or_default(),
        human_action_label: decision
            .selected_action_label
            .clone()
            .or_else(|| {
                decision
                    .selected_action_id
                    .map(|action_id| format!("Action #{action_id}"))
            })
            .unwrap_or_else(|| "Unknown action".into()),
        recommended_action_label: decision.recommended_action_label.clone().or_else(|| {
            decision
                .recommended_action_id
                .map(|action_id| format!("Action #{action_id}"))
        }),
        human_tile: None,
        recommended_tile: None,
        category: decision
            .category
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        reason: decision.recommendation_reason.clone(),
    }
}

#[derive(Debug)]
struct ProcessBackendSessionTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

fn backend_local_session_args(
    ai_worker_cmd: Option<&str>,
    ai_worker_timeout_ms: Option<&str>,
    coach_worker_cmd: Option<&str>,
    coach_worker_timeout_ms: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "--project".to_string(),
        "backend".to_string(),
        "riichi-ai-trainer".to_string(),
        "local-session".to_string(),
    ];
    if let Some(command) = ai_worker_cmd
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.push("--ai-worker-cmd".to_string());
        args.push(command.to_string());
    }
    if let Some(timeout_ms) = ai_worker_timeout_ms
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.push("--ai-worker-timeout-ms".to_string());
        args.push(timeout_ms.to_string());
    }
    if let Some(command) = coach_worker_cmd
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.push("--coach-worker-cmd".to_string());
        args.push(command.to_string());
    }
    if let Some(timeout_ms) = coach_worker_timeout_ms
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.push("--coach-worker-timeout-ms".to_string());
        args.push(timeout_ms.to_string());
    }
    args
}

fn backend_local_session_args_from_config(
    config: &BackendLocalSessionConfig,
    env_ai_worker_cmd: Option<&str>,
    env_ai_worker_timeout_ms: Option<&str>,
    env_coach_worker_cmd: Option<&str>,
    env_coach_worker_timeout_ms: Option<&str>,
) -> Vec<String> {
    let config_cmd = config
        .ai_worker_cmd
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let config_timeout = config
        .ai_worker_timeout_ms
        .filter(|timeout_ms| *timeout_ms > 0)
        .map(|timeout_ms| timeout_ms.to_string());
    let config_coach_cmd = config
        .coach_worker_cmd
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let config_coach_timeout = config
        .coach_worker_timeout_ms
        .filter(|timeout_ms| *timeout_ms > 0)
        .map(|timeout_ms| timeout_ms.to_string());
    backend_local_session_args(
        config_cmd.or(env_ai_worker_cmd).map(str::trim),
        config_timeout.as_deref().or(env_ai_worker_timeout_ms),
        config_coach_cmd.or(env_coach_worker_cmd).map(str::trim),
        config_coach_timeout
            .as_deref()
            .or(env_coach_worker_timeout_ms),
    )
}

fn backend_local_session_args_from_env(config: &BackendLocalSessionConfig) -> Vec<String> {
    let ai_worker_cmd = env::var(AI_WORKER_CMD_ENV).ok();
    let ai_worker_timeout_ms = env::var(AI_WORKER_TIMEOUT_MS_ENV).ok();
    let coach_worker_cmd = env::var(COACH_WORKER_CMD_ENV).ok();
    let coach_worker_timeout_ms = env::var(COACH_WORKER_TIMEOUT_MS_ENV).ok();
    backend_local_session_args_from_config(
        config,
        ai_worker_cmd.as_deref(),
        ai_worker_timeout_ms.as_deref(),
        coach_worker_cmd.as_deref(),
        coach_worker_timeout_ms.as_deref(),
    )
}

impl ProcessBackendSessionTransport {
    fn spawn(config: BackendLocalSessionConfig) -> Result<Self, String> {
        Self::spawn_with_args(backend_local_session_args_from_env(&config))
    }

    fn spawn_with_args(args: Vec<String>) -> Result<Self, String> {
        let repo_root = find_repo_root().ok_or_else(|| {
            "could not locate repository root with backend/pyproject.toml".to_string()
        })?;
        let mut child = Command::new("uv")
            .args(&args)
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
    fn backend_local_session_args_include_worker_config_when_present() {
        let args = backend_local_session_args(
            Some("python backend/tests/fixtures/bots/normal_bot.py"),
            Some("30000"),
            None,
            None,
        );

        assert_eq!(
            args,
            vec![
                "run",
                "--project",
                "backend",
                "riichi-ai-trainer",
                "local-session",
                "--ai-worker-cmd",
                "python backend/tests/fixtures/bots/normal_bot.py",
                "--ai-worker-timeout-ms",
                "30000",
            ]
        );
    }

    #[test]
    fn backend_local_session_args_skip_empty_worker_config() {
        let args = backend_local_session_args(Some(""), Some(""), Some(""), Some(""));

        assert_eq!(
            args,
            vec![
                "run",
                "--project",
                "backend",
                "riichi-ai-trainer",
                "local-session",
            ]
        );
    }

    #[test]
    fn backend_local_session_args_use_saved_worker_config() {
        let config = BackendLocalSessionConfig {
            ai_worker_cmd: Some("  uv run worker  ".into()),
            ai_worker_timeout_ms: Some(30000),
            coach_worker_cmd: None,
            coach_worker_timeout_ms: None,
        };

        let args = backend_local_session_args_from_config(
            &config,
            Some("env worker"),
            Some("1000"),
            None,
            None,
        );

        assert_eq!(
            args,
            vec![
                "run",
                "--project",
                "backend",
                "riichi-ai-trainer",
                "local-session",
                "--ai-worker-cmd",
                "uv run worker",
                "--ai-worker-timeout-ms",
                "30000",
            ]
        );
    }

    #[test]
    fn backend_local_session_args_fall_back_to_env_when_saved_config_empty() {
        let config = BackendLocalSessionConfig {
            ai_worker_cmd: Some("   ".into()),
            ai_worker_timeout_ms: None,
            coach_worker_cmd: None,
            coach_worker_timeout_ms: None,
        };

        let args = backend_local_session_args_from_config(
            &config,
            Some("env worker"),
            Some("5000"),
            None,
            None,
        );

        assert_eq!(
            args,
            vec![
                "run",
                "--project",
                "backend",
                "riichi-ai-trainer",
                "local-session",
                "--ai-worker-cmd",
                "env worker",
                "--ai-worker-timeout-ms",
                "5000",
            ]
        );
    }

    #[test]
    fn backend_local_session_args_include_coach_worker_config_when_present() {
        let config = BackendLocalSessionConfig {
            ai_worker_cmd: None,
            ai_worker_timeout_ms: None,
            coach_worker_cmd: Some("  uv run coach  ".into()),
            coach_worker_timeout_ms: Some(30000),
        };

        let args = backend_local_session_args_from_config(
            &config,
            None,
            None,
            Some("env coach"),
            Some("1000"),
        );

        assert_eq!(
            args,
            vec![
                "run",
                "--project",
                "backend",
                "riichi-ai-trainer",
                "local-session",
                "--coach-worker-cmd",
                "uv run coach",
                "--coach-worker-timeout-ms",
                "30000",
            ]
        );
    }

    #[test]
    fn backend_local_session_args_fall_back_to_env_for_coach_worker() {
        let config = BackendLocalSessionConfig {
            ai_worker_cmd: None,
            ai_worker_timeout_ms: None,
            coach_worker_cmd: Some("   ".into()),
            coach_worker_timeout_ms: None,
        };

        let args = backend_local_session_args_from_config(
            &config,
            None,
            None,
            Some("env coach"),
            Some("7000"),
        );

        assert_eq!(
            args,
            vec![
                "run",
                "--project",
                "backend",
                "riichi-ai-trainer",
                "local-session",
                "--coach-worker-cmd",
                "env coach",
                "--coach-worker-timeout-ms",
                "7000",
            ]
        );
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
    fn backend_session_defaults_missing_worker_metadata_to_random_agent() {
        let transport = Box::new(FakeTransport::new(vec![ok_response(
            "tauri-1",
            "backend-1",
            view(1, "1m"),
        )]));

        let session = BackendLocalSession::start_with_transport(1, transport).unwrap();
        let latest = session.latest_view().unwrap();

        assert!(!latest.engine.worker.configured);
        assert_eq!(latest.engine.worker.agent_kind, "random_agent");
        assert_eq!(latest.engine.worker.label, "RandomAgent");
        assert_eq!(latest.engine.worker.timeout_ms, None);
    }

    #[test]
    fn backend_session_parses_worker_metadata() {
        let mut backend_view = view(1, "1m");
        backend_view["engine"]["worker"] = json!({
            "schemaVersion": 1,
            "configured": true,
            "agentKind": "worker_agent",
            "label": "JSONL worker",
            "timeoutMs": 30000
        });
        let transport = Box::new(FakeTransport::new(vec![ok_response(
            "tauri-1",
            "backend-1",
            backend_view,
        )]));

        let session = BackendLocalSession::start_with_transport(1, transport).unwrap();
        let latest = session.latest_view().unwrap();

        assert!(latest.engine.worker.configured);
        assert_eq!(latest.engine.worker.agent_kind, "worker_agent");
        assert_eq!(latest.engine.worker.label, "JSONL worker");
        assert_eq!(latest.engine.worker.timeout_ms, Some(30000));
    }

    #[test]
    fn local_table_backend_process_smoke_reports_worker_metadata() {
        let worker_cmd =
            "uv run --project backend python backend/tests/fixtures/bots/normal_bot.py";
        let args = backend_local_session_args(Some(worker_cmd), Some("5000"), None, None);
        let transport = Box::new(ProcessBackendSessionTransport::spawn_with_args(args).unwrap());

        let session = BackendLocalSession::start_with_transport(1, transport).unwrap();
        let latest = session.latest_view().unwrap();

        assert!(latest.engine.worker.configured);
        assert_eq!(latest.engine.worker.agent_kind, "worker_agent");
        assert_eq!(latest.engine.worker.label, "JSONL worker");
        assert_eq!(latest.engine.worker.timeout_ms, Some(5000));
        assert_eq!(latest.source, "backend_mapper");
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
    fn backend_session_rejects_hidden_information_inside_review_report() {
        let transport = Box::new(FakeTransport::new(vec![
            ok_response("tauri-1", "backend-1", view(1, "1m")),
            json!({
                "type": "local_session_response",
                "requestId": "tauri-2",
                "ok": true,
                "gameId": "backend-1",
                "view": null,
                "terminal": true,
                "endReason": "tsumo",
                "error": null,
                "reviewReport": {
                    "schema_version": 1,
                    "summary": {
                        "decision_count": 1,
                        "matched_recommendation_count": 1,
                        "mismatch_count": 0
                    },
                    "all_decisions": [{"wall": ["1m"]}]
                }
            }),
        ]));
        let mut session = BackendLocalSession::start_with_transport(1, transport).unwrap();

        let error = session.submit_action(1).unwrap_err();

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
    fn backend_session_maps_terminal_review_report_to_local_summary() {
        let transport = Box::new(FakeTransport::new(vec![
            ok_response("tauri-1", "backend-1", view(1, "1m")),
            json!({
                "type": "local_session_response",
                "requestId": "tauri-2",
                "ok": true,
                "gameId": "backend-1",
                "view": null,
                "terminal": true,
                "endReason": "tsumo",
                "error": null,
                "reviewReport": {
                    "schema_version": 1,
                    "summary": {
                        "decision_count": 3,
                        "matched_recommendation_count": 1,
                        "mismatch_count": 1,
                        "attention_count": 3,
                        "fallback_count": 1,
                        "unavailable_count": 1,
                        "not_ranked_count": 1
                    },
                    "top_decision_mismatches": [{
                        "turn_index": 4,
                        "selected_action_id": 2,
                        "selected_action_label": "dahai 2m",
                        "recommended_action_id": 1,
                        "recommended_action_label": "dahai 1m",
                        "category": "mismatch",
                        "recommendation_reason": "worker_recommendation"
                    }],
                    "all_decisions": [{
                        "turn_index": 0,
                        "selected_action_id": 1,
                        "selected_action_label": "dahai 1m",
                        "recommended_action_id": 1,
                        "recommended_action_label": "dahai 1m",
                        "category": "top1_match",
                        "recommendation_reason": "first_legal_action"
                    }]
                }
            }),
        ]));
        let mut session = BackendLocalSession::start_with_transport(1, transport).unwrap();

        let terminal = session.submit_action(1).unwrap();

        assert_eq!(terminal.review_summary.source, "backend_review_report");
        assert_eq!(terminal.review_summary.total_decisions, 3);
        assert_eq!(terminal.review_summary.top1_matches, 1);
        assert_eq!(terminal.review_summary.mismatch_count, 1);
        assert_eq!(terminal.review_summary.attention_count, 3);
        assert_eq!(terminal.review_summary.fallback_count, 1);
        assert_eq!(terminal.review_summary.unavailable_count, 1);
        assert_eq!(terminal.review_summary.not_ranked_count, 1);
        assert_eq!(terminal.review_summary.key_choices.len(), 1);
        assert_eq!(terminal.review_summary.key_choices[0].turn_index, 4);
        assert_eq!(
            terminal.review_summary.key_choices[0].human_action_label,
            "dahai 2m"
        );
        assert_eq!(
            terminal.review_summary.key_choices[0]
                .recommended_action_label
                .as_deref(),
            Some("dahai 1m")
        );
        assert_eq!(terminal.review_summary.key_choices[0].category, "mismatch");
        assert_eq!(
            terminal.review_summary.key_choices[0].reason.as_deref(),
            Some("worker_recommendation")
        );
    }

    #[test]
    fn backend_session_keeps_review_summary_fallback_without_review_report() {
        let transport = Box::new(FakeTransport::new(vec![
            ok_response("tauri-1", "backend-1", view(1, "1m")),
            json!({
                "type": "local_session_response",
                "requestId": "tauri-2",
                "ok": true,
                "gameId": "backend-1",
                "view": null,
                "terminal": true,
                "endReason": "closed",
                "error": null
            }),
        ]));
        let mut session = BackendLocalSession::start_with_transport(1, transport).unwrap();

        let terminal = session.submit_action(1).unwrap();

        assert_eq!(terminal.review_summary.source, "unavailable");
        assert_eq!(terminal.review_summary.total_decisions, 0);
        assert_eq!(terminal.review_summary.attention_count, 0);
        assert_eq!(terminal.review_summary.fallback_count, 0);
        assert_eq!(terminal.review_summary.unavailable_count, 0);
        assert_eq!(terminal.review_summary.not_ranked_count, 0);
        assert!(terminal.review_summary.key_choices.is_empty());
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
