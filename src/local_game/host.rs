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
    pub view: LocalGameView,
}

pub struct LocalGameHost {
    seed: u64,
}

impl LocalGameHost {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    pub fn start_session(self, game_id: String) -> LocalGameSession {
        let initial = VisibleInitialGameState::from_seed(self.seed);
        LocalGameSession {
            game_id,
            seed: self.seed,
            view: initial.into_view(),
        }
    }
}

#[derive(Debug, Clone)]
struct VisibleInitialGameState {
    self_hand_tiles: Vec<String>,
    dora_indicator: String,
    remaining_tiles: u32,
}

impl VisibleInitialGameState {
    fn from_seed(seed: u64) -> Self {
        let mut wall = tile_wall();
        let offset = (seed as usize) % wall.len();
        wall.rotate_left(offset);

        let self_hand_tiles = wall.iter().take(14).cloned().collect();
        let dora_indicator = wall.get(53).cloned().unwrap_or_else(|| "5m".into());

        Self {
            self_hand_tiles,
            dora_indicator,
            remaining_tiles: 70,
        }
    }

    fn into_view(self) -> LocalGameView {
        let actions = discard_actions(&self.self_hand_tiles);
        let recommendation = recommendation_from_first_action(&actions);
        LocalGameView {
            schema_version: 1,
            source: "local_game_host".into(),
            phase_label: "Initial local hand".into(),
            notice: "Local game session view. Action submit is not wired yet.".into(),
            round: LocalGameRoundView {
                round_label: "E1".into(),
                honba: 0,
                kyotaku: 0,
                remaining_tiles: self.remaining_tiles,
                dealer_seat: 0,
            },
            players: player_views(),
            self_hand_tiles: self.self_hand_tiles,
            dora_indicators: vec![self.dora_indicator],
            actions,
            recommendations: vec![recommendation],
        }
    }
}

fn player_views() -> Vec<LocalGamePlayerView> {
    (0..4)
        .map(|seat| LocalGamePlayerView {
            seat,
            relation_label: RELATION_LABELS[seat as usize].into(),
            wind: WINDS[seat as usize].into(),
            score: 25000,
            river_tiles: vec![],
            melds: vec![],
            status_tags: if seat == 0 {
                vec!["thinking".into()]
            } else {
                vec![]
            },
            is_dealer: seat == 0,
            is_self: seat == 0,
        })
        .collect()
}

fn discard_actions(self_hand_tiles: &[String]) -> Vec<LocalGameActionView> {
    self_hand_tiles
        .iter()
        .enumerate()
        .map(|(index, tile)| LocalGameActionView {
            id: (index + 1) as u32,
            action_type: "discard".into(),
            label: format!("Discard {tile}"),
            hint: "Action submit is not wired yet".into(),
            enabled: false,
            tile: Some(tile.clone()),
            mjai: Some(json!({"type": "dahai", "actor": 0, "pai": tile})),
        })
        .collect()
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

    #[test]
    fn local_game_host_builds_non_fixture_initial_view() {
        let session = LocalGameHost::new(1).start_session("local-test".into());

        assert_eq!(session.game_id, "local-test");
        assert_eq!(session.view.source, "local_game_host");
        assert_ne!(session.view.source, "tauri_fixture");
        assert_eq!(session.view.players.len(), 4);
        assert_eq!(session.view.self_hand_tiles.len(), 14);
        assert!(!session.view.dora_indicators.is_empty());
        assert!(!session.view.actions.is_empty());
    }

    #[test]
    fn local_game_host_view_does_not_expose_hidden_fields() {
        let session = LocalGameHost::new(1).start_session("local-test".into());
        let value = serde_json::to_value(&session.view).unwrap();
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
    fn local_game_host_recommendations_reference_existing_actions() {
        let session = LocalGameHost::new(1).start_session("local-test".into());
        let action_ids = session
            .view
            .actions
            .iter()
            .map(|action| action.id)
            .collect::<BTreeSet<_>>();

        for recommendation in session.view.recommendations {
            if let Some(action_id) = recommendation.action_id {
                assert!(action_ids.contains(&action_id));
            }
        }
    }
}
