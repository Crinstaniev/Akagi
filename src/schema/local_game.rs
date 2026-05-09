use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGameSessionHandle {
    pub game_id: String,
    pub view: LocalGameView,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGameView {
    pub schema_version: u32,
    pub source: String,
    pub phase_label: String,
    pub notice: String,
    pub round: LocalGameRoundView,
    pub players: Vec<LocalGamePlayerView>,
    pub self_hand_tiles: Vec<String>,
    pub dora_indicators: Vec<String>,
    pub actions: Vec<LocalGameActionView>,
    pub recommendations: Vec<LocalGameRecommendationView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGameRoundView {
    pub round_label: String,
    pub honba: u32,
    pub kyotaku: u32,
    pub remaining_tiles: u32,
    pub dealer_seat: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGamePlayerView {
    pub seat: u8,
    pub relation_label: String,
    pub wind: String,
    pub score: i32,
    pub river_tiles: Vec<String>,
    pub melds: Vec<Vec<String>>,
    pub status_tags: Vec<String>,
    pub is_dealer: bool,
    pub is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGameActionView {
    pub id: u32,
    #[serde(rename = "type")]
    pub action_type: String,
    pub label: String,
    pub hint: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mjai: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGameRecommendationView {
    pub rank: u32,
    pub action_id: Option<u32>,
    pub tile: Option<String>,
    pub label: String,
    pub source: String,
    pub status: String,
    pub note: String,
}

pub fn local_game_session_fixture() -> LocalGameSessionHandle {
    LocalGameSessionHandle {
        game_id: "local-fixture-001".into(),
        view: local_game_view_fixture(),
    }
}

pub fn local_game_view_fixture() -> LocalGameView {
    LocalGameView {
        schema_version: 1,
        source: "tauri_fixture".into(),
        phase_label: "Tauri command fixture".into(),
        notice: "Tauri command fixture only. Not live LocalGameHost state.".into(),
        round: LocalGameRoundView {
            round_label: "E1".into(),
            honba: 0,
            kyotaku: 0,
            remaining_tiles: 62,
            dealer_seat: 0,
        },
        players: vec![
            LocalGamePlayerView {
                seat: 0,
                relation_label: "自家".into(),
                wind: "E".into(),
                score: 25000,
                river_tiles: vec!["1m".into(), "2p".into(), "9s".into()],
                melds: vec![],
                status_tags: vec!["thinking".into()],
                is_dealer: true,
                is_self: true,
            },
            LocalGamePlayerView {
                seat: 1,
                relation_label: "下家".into(),
                wind: "S".into(),
                score: 25000,
                river_tiles: vec!["3m".into(), "7p".into(), "P".into()],
                melds: vec![vec!["P".into(), "P".into(), "P".into()]],
                status_tags: vec![],
                is_dealer: false,
                is_self: false,
            },
            LocalGamePlayerView {
                seat: 2,
                relation_label: "对面".into(),
                wind: "W".into(),
                score: 25000,
                river_tiles: vec!["9m".into(), "2s".into(), "C".into()],
                melds: vec![],
                status_tags: vec![],
                is_dealer: false,
                is_self: false,
            },
            LocalGamePlayerView {
                seat: 3,
                relation_label: "上家".into(),
                wind: "N".into(),
                score: 25000,
                river_tiles: vec!["4m".into(), "8p".into(), "F".into()],
                melds: vec![vec!["3p".into(), "3p".into(), "3p".into()]],
                status_tags: vec![],
                is_dealer: false,
                is_self: false,
            },
        ],
        self_hand_tiles: vec!["123m".into(), "405p".into(), "678s".into(), "11z".into()],
        dora_indicators: vec!["5m".into()],
        actions: vec![
            LocalGameActionView {
                id: 1,
                action_type: "discard".into(),
                label: "Discard 5p".into(),
                hint: "Command fixture action".into(),
                enabled: false,
                tile: Some("5p".into()),
                mjai: Some(json!({"type": "dahai", "actor": 0, "pai": "5p"})),
            },
            LocalGameActionView {
                id: 2,
                action_type: "riichi".into(),
                label: "Riichi".into(),
                hint: "Not wired yet".into(),
                enabled: false,
                tile: None,
                mjai: None,
            },
            LocalGameActionView {
                id: 3,
                action_type: "skip".into(),
                label: "Skip".into(),
                hint: "Not wired yet".into(),
                enabled: false,
                tile: None,
                mjai: Some(json!({"type": "none", "actor": 0})),
            },
        ],
        recommendations: vec![LocalGameRecommendationView {
            rank: 1,
            action_id: Some(1),
            tile: Some("5p".into()),
            label: "Recommended discard 5p".into(),
            source: "tauri_fixture".into(),
            status: "recommended".into(),
            note: "Command fixture only; not a real AI recommendation.".into(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn local_game_fixture_serializes_required_fields() {
        let handle = local_game_session_fixture();
        let value = serde_json::to_value(&handle).expect("serialize local game fixture");

        assert_eq!(value["gameId"], "local-fixture-001");
        assert_eq!(value["view"]["schemaVersion"], 1);
        assert_eq!(value["view"]["source"], "tauri_fixture");
        assert_eq!(value["view"]["players"].as_array().unwrap().len(), 4);
        assert!(value["view"]["selfHandTiles"].as_array().unwrap().len() > 0);
        assert!(value["view"]["actions"].as_array().unwrap().len() > 0);
        assert!(value["view"]["recommendations"].as_array().unwrap().len() > 0);
    }

    #[test]
    fn local_game_fixture_does_not_expose_hidden_fields() {
        let value = serde_json::to_value(local_game_view_fixture()).unwrap();
        let mut keys = BTreeSet::new();
        collect_keys(&value, &mut keys);

        for forbidden in [
            "hands",
            "hidden_state",
            "wall",
            "handTiles",
            "hiddenTiles",
            "wallTiles",
        ] {
            assert!(
                !keys.contains(forbidden),
                "forbidden key leaked: {forbidden}"
            );
        }
    }

    #[test]
    fn local_game_recommendations_reference_existing_actions() {
        let view = local_game_view_fixture();
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
}
