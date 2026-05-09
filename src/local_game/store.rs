use super::host::{LocalGameHost, LocalGameSession};
use crate::schema::{LocalGameSessionHandle, LocalGameView};
use std::collections::BTreeMap;
use ulid::Ulid;

#[derive(Debug, Default)]
pub struct LocalGameSessionStore {
    sessions: BTreeMap<String, LocalGameSession>,
    next_seed: u64,
}

impl LocalGameSessionStore {
    pub fn new() -> Self {
        Self {
            sessions: BTreeMap::new(),
            next_seed: 1,
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
        self.sessions
            .get_mut(trimmed)
            .ok_or_else(|| format!("local game session not found: {trimmed}"))?
            .submit_action(action_id)
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

        assert_eq!(
            after.self_hand_tiles.len(),
            before.self_hand_tiles.len() - 1
        );
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
}
