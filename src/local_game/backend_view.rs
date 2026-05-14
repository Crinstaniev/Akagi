use crate::schema::local_game::{LocalGameView, LocalReviewSummary};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const FORBIDDEN_OUTPUT_KEYS: [&str; 6] = [
    "hands",
    "hidden_state",
    "wall",
    "handTiles",
    "hiddenTiles",
    "wallTiles",
];

pub fn load_backend_initial_view(seed: u64) -> Result<LocalGameView, String> {
    let repo_root = find_repo_root().ok_or_else(|| {
        "could not locate repository root with backend/pyproject.toml".to_string()
    })?;
    let output = Command::new("uv")
        .args([
            "run",
            "--project",
            "backend",
            "riichi-ai-trainer",
            "local-view",
            "--game-mode",
            "4p-red-single",
            "--seed",
            &seed.to_string(),
        ])
        .current_dir(&repo_root)
        .output()
        .map_err(|error| format!("failed to run backend local-view: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "backend local-view exited with status {}; {stderr}",
            output.status
        ));
    }

    parse_backend_initial_view(&output.stdout)
}

pub fn parse_backend_initial_view(bytes: &[u8]) -> Result<LocalGameView, String> {
    if bytes.is_empty() {
        return Err("backend local-view stdout is empty".into());
    }
    let raw: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("backend local-view JSON parse failed: {error}"))?;
    let mut view = parse_backend_view_value(raw)?;
    mark_backend_view_read_only(&mut view);
    Ok(view)
}

pub fn parse_backend_view_value(raw: Value) -> Result<LocalGameView, String> {
    reject_forbidden_output_keys(&raw)?;
    let view: LocalGameView = serde_json::from_value(raw)
        .map_err(|error| format!("backend LocalGameView schema parse failed: {error}"))?;
    validate_backend_view(&view)?;
    Ok(view)
}

pub fn reject_forbidden_output_keys(raw: &Value) -> Result<(), String> {
    let leaked = forbidden_keys_in_value(raw);
    if leaked.is_empty() {
        return Ok(());
    }
    Err(format!(
        "backend LocalGameView contains hidden-information keys: {:?}",
        leaked
    ))
}

pub fn backend_fallback_notice(error: &str) -> String {
    format!("Backend local-view unavailable; using deterministic fallback. {error}")
}

fn validate_backend_view(view: &LocalGameView) -> Result<(), String> {
    if view.schema_version != 1 {
        return Err(format!(
            "unsupported backend LocalGameView schemaVersion: {}",
            view.schema_version
        ));
    }
    if view.source.trim().is_empty() {
        return Err("backend LocalGameView source is required".into());
    }
    if view.engine.source.trim().is_empty() {
        return Err("backend LocalGameView engine.source is required".into());
    }
    if view.players.len() != 4 {
        return Err("backend LocalGameView players must contain four seats".into());
    }
    Ok(())
}

fn mark_backend_view_read_only(view: &mut LocalGameView) {
    view.notice = format!(
        "{} Submit bridge is not wired yet; this backend initial view is read-only.",
        view.notice
    );
    for action in &mut view.actions {
        action.enabled = false;
        action.hint = "Backend initial view only; submit bridge is not wired yet.".into();
    }
}

fn forbidden_keys_in_value(value: &Value) -> Vec<String> {
    let mut keys = BTreeSet::new();
    collect_keys(value, &mut keys);
    FORBIDDEN_OUTPUT_KEYS
        .iter()
        .filter(|key| keys.contains(**key))
        .map(|key| (*key).into())
        .collect()
}

fn collect_keys(value: &Value, keys: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                keys.insert(key.clone());
                collect_keys(child, keys);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_keys(item, keys);
            }
        }
        _ => {}
    }
}

pub fn find_repo_root() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        if is_repo_root(&current) {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn is_repo_root(path: &Path) -> bool {
    path.join("backend").join("pyproject.toml").exists()
}

impl Default for LocalReviewSummary {
    fn default() -> Self {
        Self {
            schema_version: 1,
            source: "unavailable".into(),
            total_decisions: 0,
            top1_matches: 0,
            mismatch_count: 0,
            attention_count: 0,
            fallback_count: 0,
            unavailable_count: 0,
            not_ranked_count: 0,
            key_choices: Vec::new(),
            note: "Review summary is not available for backend initial view.".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::LocalArtifactStatus;

    fn backend_view_json() -> Vec<u8> {
        r#"{
          "schemaVersion": 1,
          "source": "backend_mapper",
          "engine": {
            "schemaVersion": 1,
            "source": "backend_riichienv_mapper",
            "status": "active",
            "capabilities": ["discard"],
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
          "selfHandTiles": ["1m"],
          "doraIndicators": ["5m"],
          "actions": [{"id": 1, "type": "discard", "label": "dahai 1m", "hint": "合法动作", "enabled": true, "tile": "1m"}],
          "recommendations": [{"rank": 1, "actionId": 1, "tile": "1m", "label": "dahai 1m", "source": "deterministic_baseline", "status": "recommended", "note": "first"}]
        }"#
        .as_bytes()
        .to_vec()
    }

    #[test]
    fn parses_backend_initial_view_and_marks_actions_read_only() {
        let view = parse_backend_initial_view(&backend_view_json()).unwrap();

        assert_eq!(view.source, "backend_mapper");
        assert_eq!(view.engine.source, "backend_riichienv_mapper");
        assert!(!view.actions[0].enabled);
        assert!(view.notice.contains("read-only"));
        assert_eq!(view.artifact_status, LocalArtifactStatus::pending());
        assert_eq!(view.review_summary.total_decisions, 0);
        assert_eq!(view.review_summary.unavailable_count, 0);
        assert_eq!(view.review_summary.not_ranked_count, 0);
    }

    #[test]
    fn rejects_invalid_backend_json() {
        let error = parse_backend_initial_view(b"not json").unwrap_err();

        assert!(error.contains("JSON parse failed"));
    }

    #[test]
    fn rejects_hidden_information_keys() {
        let mut value: Value = serde_json::from_slice(&backend_view_json()).unwrap();
        value["hiddenTiles"] = Value::Array(vec![]);
        let bytes = serde_json::to_vec(&value).unwrap();

        let error = parse_backend_initial_view(&bytes).unwrap_err();

        assert!(error.contains("hidden-information"));
    }
}
