use super::host::{LocalGameHost, LocalGameSession};
use crate::schema::{LocalGameSessionHandle, LocalGameView};
use std::collections::BTreeMap;
use std::path::PathBuf;
use ulid::Ulid;

#[derive(Debug, Default)]
pub struct LocalGameSessionStore {
    sessions: BTreeMap<String, LocalGameSession>,
    next_seed: u64,
    artifact_root: Option<PathBuf>,
}

impl LocalGameSessionStore {
    pub fn new() -> Self {
        Self {
            sessions: BTreeMap::new(),
            next_seed: 1,
            artifact_root: None,
        }
    }

    pub fn with_artifact_root(artifact_root: PathBuf) -> Self {
        Self {
            sessions: BTreeMap::new(),
            next_seed: 1,
            artifact_root: Some(artifact_root),
        }
    }

    pub fn new_session(&mut self) -> LocalGameSessionHandle {
        let game_id = format!("local-{}", Ulid::new());
        let seed = self.next_seed;
        self.next_seed += 1;

        let session = LocalGameHost::new(seed).start_session(game_id.clone());
        let view = session.view();
        self.sessions.insert(game_id.clone(), session);

        LocalGameSessionHandle { game_id, view }
    }

    pub fn get_view(&self, game_id: &str) -> Result<LocalGameView, String> {
        let trimmed = game_id.trim();
        if trimmed.is_empty() {
            return Err("gameId is required".into());
        }
        self.sessions
            .get(trimmed)
            .map(|session| session.view())
            .ok_or_else(|| format!("local game session not found: {trimmed}"))
    }

    pub fn submit_action(
        &mut self,
        game_id: &str,
        action_id: u32,
    ) -> Result<LocalGameView, String> {
        let trimmed = game_id.trim();
        if trimmed.is_empty() {
            return Err("gameId is required".into());
        }
        let session = self
            .sessions
            .get_mut(trimmed)
            .ok_or_else(|| format!("local game session not found: {trimmed}"))?;
        let view = session.submit_action(action_id)?;
        if view.phase_label == "Exhaustive draw" {
            if let Some(root) = &self.artifact_root {
                session.persist_artifacts(root);
                return Ok(session.view());
            }
        }
        Ok(view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_returns_non_fixture_source_and_complete_view() {
        let mut store = LocalGameSessionStore::new();
        let handle = store.new_session();

        assert!(handle.game_id.starts_with("local-"));
        assert_eq!(handle.view.source, "local_game_host");
        assert_ne!(handle.view.source, "tauri_fixture");
        assert_eq!(handle.view.players.len(), 4);
        assert_eq!(handle.view.self_hand_tiles.len(), 14);
        assert!(!handle.view.dora_indicators.is_empty());
        assert!(!handle.view.actions.is_empty());
        assert!(!handle.view.recommendations.is_empty());
    }

    #[test]
    fn get_view_returns_same_session_view() {
        let mut store = LocalGameSessionStore::new();
        let handle = store.new_session();

        let view = store.get_view(&handle.game_id).unwrap();

        assert_eq!(view, handle.view);
    }

    #[test]
    fn get_view_rejects_unknown_or_empty_game_id() {
        let mut store = LocalGameSessionStore::new();
        store.new_session();

        assert!(store.get_view("").is_err());
        assert!(store.get_view("  ").is_err());
        assert!(store.get_view("missing").is_err());
    }

    #[test]
    fn submit_action_returns_updated_view() {
        let mut store = LocalGameSessionStore::new();
        let handle = store.new_session();
        let before = handle.view;

        let after = store
            .submit_action(&handle.game_id, before.actions[0].id)
            .unwrap();

        assert_eq!(after.self_hand_tiles.len(), before.self_hand_tiles.len());
        assert_eq!(
            after
                .players
                .iter()
                .find(|player| player.is_self)
                .unwrap()
                .river_tiles
                .len(),
            1
        );
        assert!(after.actions.iter().any(|action| action.enabled));
        assert_eq!(
            after.round.remaining_tiles,
            before.round.remaining_tiles - 4
        );
        assert!(after
            .players
            .iter()
            .filter(|player| !player.is_self)
            .all(|player| player.river_tiles.len() == 1));
    }

    #[test]
    fn submit_action_rejects_unknown_session() {
        let mut store = LocalGameSessionStore::new();

        assert!(store.submit_action("", 1).is_err());
        assert!(store.submit_action("missing", 1).is_err());
    }

    #[test]
    fn submit_action_error_does_not_modify_session() {
        let mut store = LocalGameSessionStore::new();
        let handle = store.new_session();

        assert!(store.submit_action(&handle.game_id, 999).is_err());

        assert_eq!(store.get_view(&handle.game_id).unwrap(), handle.view);
    }

    #[test]
    fn terminal_submit_writes_local_artifacts_when_root_configured() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut store = LocalGameSessionStore::with_artifact_root(tmp.path().to_path_buf());
        let handle = store.new_session();
        let game_id = handle.game_id.clone();

        let mut view = handle.view;
        while view.phase_label != "Exhaustive draw" {
            view = store
                .submit_action(&game_id, view.actions[0].id)
                .expect("submit action");
        }

        assert!(view.artifact_status.saved);
        let replay_path = view.artifact_status.replay_path.as_ref().unwrap();
        let decision_points_path = view.artifact_status.decision_points_path.as_ref().unwrap();
        assert!(std::path::Path::new(replay_path).exists());
        assert!(std::path::Path::new(decision_points_path).exists());

        let replay: serde_json::Value =
            serde_json::from_slice(&std::fs::read(replay_path).unwrap()).unwrap();
        let decision_points: serde_json::Value =
            serde_json::from_slice(&std::fs::read(decision_points_path).unwrap()).unwrap();
        assert_eq!(replay["finalPhaseLabel"], "Exhaustive draw");
        assert_eq!(
            replay["decisionCount"],
            decision_points["decisionPoints"].as_array().unwrap().len()
        );
    }

    #[test]
    fn terminal_submit_persists_artifacts_idempotently() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut store = LocalGameSessionStore::with_artifact_root(tmp.path().to_path_buf());
        let handle = store.new_session();
        let game_id = handle.game_id.clone();

        let mut view = handle.view;
        while view.phase_label != "Exhaustive draw" {
            view = store.submit_action(&game_id, view.actions[0].id).unwrap();
        }
        let first = view.artifact_status.clone();
        let second = store.get_view(&game_id).unwrap().artifact_status;

        assert_eq!(first, second);
        assert!(second.saved);
    }
}
