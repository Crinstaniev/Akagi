use crate::schema::local_game::{
    LocalGameActionView, LocalGamePlayerView, LocalGameRecommendationView, LocalGameRoundView,
    LocalGameView,
};
use serde_json::json;

const RELATION_LABELS: [&str; 4] = ["自家", "下家", "对面", "上家"];
const WINDS: [&str; 4] = ["E", "S", "W", "N"];
#[cfg(test)]
const FORBIDDEN_OUTPUT_KEYS: [&str; 6] = [
    "hands",
    "hidden_state",
    "wall",
    "handTiles",
    "hiddenTiles",
    "wallTiles",
];

#[derive(Debug, Clone)]
pub struct LocalGameSession {
    pub game_id: String,
    pub seed: u64,
    state: LocalGameSessionState,
}

impl LocalGameSession {
    pub fn view(&self) -> LocalGameView {
        self.state.view()
    }

    pub fn submit_action(&mut self, action_id: u32) -> Result<LocalGameView, String> {
        self.state.submit_action(action_id)?;
        Ok(self.view())
    }
}

pub struct LocalGameHost {
    seed: u64,
}

impl LocalGameHost {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    pub fn start_session(self, game_id: String) -> LocalGameSession {
        LocalGameSession {
            game_id,
            seed: self.seed,
            state: LocalGameSessionState::from_seed(self.seed),
        }
    }
}

#[derive(Debug, Clone)]
struct LocalGameSessionState {
    self_hand_tiles: Vec<String>,
    player_river_tiles: Vec<Vec<String>>,
    opponent_discard_tiles: Vec<String>,
    dora_indicator: String,
    remaining_tiles: u32,
    turn_index: u32,
    actions_enabled: bool,
}

impl LocalGameSessionState {
    fn from_seed(seed: u64) -> Self {
        let mut wall = tile_wall();
        let offset = (seed as usize) % wall.len();
        wall.rotate_left(offset);

        let self_hand_tiles = wall.iter().take(14).cloned().collect();
        let opponent_discard_tiles = wall.iter().skip(14).take(72).cloned().collect();
        let dora_indicator = wall.get(53).cloned().unwrap_or_else(|| "5m".into());

        Self {
            self_hand_tiles,
            player_river_tiles: vec![vec![], vec![], vec![], vec![]],
            opponent_discard_tiles,
            dora_indicator,
            remaining_tiles: 70,
            turn_index: 0,
            actions_enabled: true,
        }
    }

    fn view(&self) -> LocalGameView {
        let actions = self.discard_actions();
        let recommendation = recommendation_from_first_action(&actions);
        LocalGameView {
            schema_version: 1,
            source: "local_game_host".into(),
            phase_label: if self.turn_index == 0 {
                "Initial local hand".into()
            } else {
                "Waiting for your action".into()
            },
            notice: if self.turn_index == 0 {
                "Local game session view. Submit a discard to update this session.".into()
            } else {
                "Deterministic AI auto-advance completed. CoachWorker is not wired yet.".into()
            },
            round: LocalGameRoundView {
                round_label: "E1".into(),
                honba: 0,
                kyotaku: 0,
                remaining_tiles: self.remaining_tiles,
                dealer_seat: 0,
            },
            players: player_views(&self.player_river_tiles),
            self_hand_tiles: self.self_hand_tiles.clone(),
            dora_indicators: vec![self.dora_indicator.clone()],
            actions,
            recommendations: vec![recommendation],
        }
    }

    fn submit_action(&mut self, action_id: u32) -> Result<(), String> {
        let actions = self.discard_actions();
        let action = actions
            .iter()
            .find(|action| action.id == action_id)
            .ok_or_else(|| format!("local game action not found: {action_id}"))?;
        if !action.enabled {
            return Err(format!("local game action is disabled: {action_id}"));
        }
        if action.action_type != "discard" {
            return Err(format!(
                "unsupported local game action type: {}",
                action.action_type
            ));
        }

        let index = action_id
            .checked_sub(1)
            .map(|value| value as usize)
            .ok_or_else(|| format!("local game action not found: {action_id}"))?;
        if index >= self.self_hand_tiles.len() {
            return Err(format!("local game action not found: {action_id}"));
        }

        let tile = self.self_hand_tiles.remove(index);
        self.player_river_tiles[0].push(tile);
        self.remaining_tiles = self.remaining_tiles.saturating_sub(1);
        self.auto_advance_opponents();
        self.turn_index += 1;
        Ok(())
    }

    fn auto_advance_opponents(&mut self) {
        for seat in 1..=3 {
            let tile = self
                .opponent_discard_tiles
                .pop()
                .unwrap_or_else(|| fallback_opponent_tile(seat, self.turn_index));
            self.player_river_tiles[seat].push(tile);
            self.remaining_tiles = self.remaining_tiles.saturating_sub(1);
        }
    }

    fn discard_actions(&self) -> Vec<LocalGameActionView> {
        self.self_hand_tiles
            .iter()
            .enumerate()
            .map(|(index, tile)| LocalGameActionView {
                id: (index + 1) as u32,
                action_type: "discard".into(),
                label: format!("Discard {tile}"),
                hint: "Submit discard".into(),
                enabled: self.actions_enabled,
                tile: Some(tile.clone()),
                mjai: Some(json!({"type": "dahai", "actor": 0, "pai": tile})),
            })
            .collect()
    }
}

fn player_views(player_river_tiles: &[Vec<String>]) -> Vec<LocalGamePlayerView> {
    (0..4)
        .map(|seat| LocalGamePlayerView {
            seat,
            relation_label: RELATION_LABELS[seat as usize].into(),
            wind: WINDS[seat as usize].into(),
            score: 25000,
            river_tiles: player_river_tiles
                .get(seat as usize)
                .cloned()
                .unwrap_or_default(),
            melds: vec![],
            status_tags: if seat == 0 {
                vec!["thinking".into()]
            } else if player_river_tiles
                .get(seat as usize)
                .is_some_and(|river| !river.is_empty())
            {
                vec!["auto-advanced".into()]
            } else {
                vec![]
            },
            is_dealer: seat == 0,
            is_self: seat == 0,
        })
        .collect()
}

fn fallback_opponent_tile(seat: usize, turn_index: u32) -> String {
    const FALLBACK_TILES: [&str; 9] = ["1p", "2p", "3p", "4s", "5s", "6s", "E", "S", "W"];
    let index = (seat + turn_index as usize) % FALLBACK_TILES.len();
    FALLBACK_TILES[index].into()
}

fn recommendation_from_first_action(
    actions: &[LocalGameActionView],
) -> LocalGameRecommendationView {
    let action = actions.first();
    LocalGameRecommendationView {
        rank: 1,
        action_id: action.map(|action| action.id),
        tile: action.and_then(|action| action.tile.clone()),
        label: action
            .map(|action| format!("Baseline {}", action.label.to_lowercase()))
            .unwrap_or_else(|| "No recommendation".into()),
        source: "deterministic_baseline".into(),
        status: if action.is_some() {
            "recommended".into()
        } else {
            "unavailable".into()
        },
        note: "Deterministic placeholder until CoachWorker is wired.".into(),
    }
}

fn tile_wall() -> Vec<String> {
    let mut tiles = Vec::with_capacity(136);
    for suit in ["m", "p", "s"] {
        for rank in 1..=9 {
            for _ in 0..4 {
                tiles.push(format!("{rank}{suit}"));
            }
        }
    }
    for honor in ["E", "S", "W", "N", "P", "F", "C"] {
        for _ in 0..4 {
            tiles.push(honor.to_string());
        }
    }
    tiles
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::BTreeSet;

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

    fn self_river_len(view: &LocalGameView) -> usize {
        view.players
            .iter()
            .find(|player| player.is_self)
            .unwrap()
            .river_tiles
            .len()
    }

    fn river_len(view: &LocalGameView, seat: u8) -> usize {
        view.players
            .iter()
            .find(|player| player.seat == seat)
            .unwrap()
            .river_tiles
            .len()
    }

    fn total_river_len(view: &LocalGameView) -> usize {
        view.players
            .iter()
            .map(|player| player.river_tiles.len())
            .sum()
    }

    #[test]
    fn local_game_host_builds_non_fixture_initial_view() {
        let session = LocalGameHost::new(1).start_session("local-test".into());
        let view = session.view();

        assert_eq!(session.game_id, "local-test");
        assert_eq!(view.source, "local_game_host");
        assert_ne!(view.source, "tauri_fixture");
        assert_eq!(view.players.len(), 4);
        assert_eq!(view.self_hand_tiles.len(), 14);
        assert!(!view.dora_indicators.is_empty());
        assert!(view.actions.iter().all(|action| action.enabled));
    }

    #[test]
    fn submit_discard_updates_hand_and_river() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());
        let before = session.view();
        let discarded = before.actions[0].tile.clone().unwrap();

        let after = session.submit_action(before.actions[0].id).unwrap();

        assert_eq!(
            after.self_hand_tiles.len(),
            before.self_hand_tiles.len() - 1
        );
        let self_player = after.players.iter().find(|player| player.is_self).unwrap();
        assert_eq!(self_player.river_tiles, vec![discarded]);
        assert_eq!(after.actions.len(), before.actions.len() - 1);
        assert_eq!(after.phase_label, "Waiting for your action");
        assert!(after.notice.contains("auto-advance completed"));
    }

    #[test]
    fn submit_auto_advances_each_opponent_river() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());
        let before = session.view();

        let after = session.submit_action(before.actions[0].id).unwrap();

        assert_eq!(river_len(&after, 0), river_len(&before, 0) + 1);
        assert_eq!(river_len(&after, 1), river_len(&before, 1) + 1);
        assert_eq!(river_len(&after, 2), river_len(&before, 2) + 1);
        assert_eq!(river_len(&after, 3), river_len(&before, 3) + 1);
        assert_eq!(total_river_len(&after), total_river_len(&before) + 4);
    }

    #[test]
    fn submit_auto_advance_decrements_remaining_tiles_by_four() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());
        let before = session.view();

        let after = session.submit_action(before.actions[0].id).unwrap();

        assert_eq!(
            after.round.remaining_tiles,
            before.round.remaining_tiles - 4
        );
        assert!(after.actions.iter().any(|action| action.enabled));
    }

    #[test]
    fn submit_unknown_action_preserves_view() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());
        let before = session.view();

        assert!(session.submit_action(999).is_err());

        assert_eq!(session.view(), before);
    }

    #[test]
    fn submit_unknown_action_does_not_auto_advance() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());
        let before = session.view();

        assert!(session.submit_action(999).is_err());

        assert_eq!(total_river_len(&session.view()), total_river_len(&before));
    }

    #[test]
    fn submit_disabled_action_preserves_view() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());
        session.state.actions_enabled = false;
        let before = session.view();

        let error = session.submit_action(before.actions[0].id).unwrap_err();

        assert!(error.contains("disabled"));
        assert_eq!(session.view(), before);
        assert_eq!(total_river_len(&session.view()), total_river_len(&before));
    }

    #[test]
    fn submit_view_does_not_expose_hidden_fields() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());
        let view = session.submit_action(1).unwrap();
        let value = serde_json::to_value(&view).unwrap();
        let mut keys = BTreeSet::new();
        collect_keys(&value, &mut keys);

        for forbidden in FORBIDDEN_OUTPUT_KEYS {
            assert!(
                !keys.contains(forbidden),
                "forbidden key leaked: {forbidden}"
            );
        }
    }

    #[test]
    fn submit_recommendations_reference_existing_actions() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());
        let view = session.submit_action(1).unwrap();
        let action_ids = view
            .actions
            .iter()
            .map(|action| action.id)
            .collect::<BTreeSet<_>>();

        for recommendation in view.recommendations {
            if let Some(action_id) = recommendation.action_id {
                assert!(action_ids.contains(&action_id));
            }
        }
    }

    #[test]
    fn initial_view_does_not_expose_hidden_fields() {
        let session = LocalGameHost::new(1).start_session("local-test".into());
        let value = serde_json::to_value(session.view()).unwrap();
        let mut keys = BTreeSet::new();
        collect_keys(&value, &mut keys);

        for forbidden in FORBIDDEN_OUTPUT_KEYS {
            assert!(
                !keys.contains(forbidden),
                "forbidden key leaked: {forbidden}"
            );
        }
    }

    #[test]
    fn initial_recommendations_reference_existing_actions() {
        let session = LocalGameHost::new(1).start_session("local-test".into());
        let view = session.view();
        let action_ids = view
            .actions
            .iter()
            .map(|action| action.id)
            .collect::<BTreeSet<_>>();

        for recommendation in view.recommendations {
            if let Some(action_id) = recommendation.action_id {
                assert!(action_ids.contains(&action_id));
            }
        }
    }

    #[test]
    fn self_river_len_helper_tracks_self_player() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());
        assert_eq!(self_river_len(&session.view()), 0);
        assert_eq!(self_river_len(&session.submit_action(1).unwrap()), 1);
    }
}
