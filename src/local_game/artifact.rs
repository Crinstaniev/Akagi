use crate::schema::local_game::{
    LocalGameActionView, LocalGamePlayerView, LocalGameRecommendationView, LocalGameRoundView,
    LocalGameView,
};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const LOCAL_ARTIFACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalArtifactPaths {
    pub replay_path: PathBuf,
    pub decision_points_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalReplayArtifact {
    pub schema_version: u32,
    pub game_id: String,
    pub source: String,
    pub seed: u64,
    pub final_phase_label: String,
    pub final_round: LocalGameRoundView,
    pub final_players: Vec<LocalGamePlayerView>,
    pub final_self_hand_tiles: Vec<String>,
    pub dora_indicators: Vec<String>,
    pub decision_count: usize,
    pub saved_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDecisionPointArtifact {
    pub schema_version: u32,
    pub game_id: String,
    pub source: String,
    pub decision_points: Vec<LocalDecisionPoint>,
    pub saved_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDecisionPoint {
    pub turn_index: u32,
    pub action_id: u32,
    pub action_label: String,
    pub action_type: String,
    pub tile: Option<String>,
    pub round: LocalGameRoundView,
    pub self_hand_tiles: Vec<String>,
    pub players: Vec<LocalGamePlayerView>,
    pub dora_indicators: Vec<String>,
    pub actions: Vec<LocalGameActionView>,
    pub recommendations: Vec<LocalGameRecommendationView>,
}

impl LocalDecisionPoint {
    pub fn from_view(turn_index: u32, view: &LocalGameView, action: &LocalGameActionView) -> Self {
        Self {
            turn_index,
            action_id: action.id,
            action_label: action.label.clone(),
            action_type: action.action_type.clone(),
            tile: action.tile.clone(),
            round: view.round.clone(),
            self_hand_tiles: view.self_hand_tiles.clone(),
            players: view.players.clone(),
            dora_indicators: view.dora_indicators.clone(),
            actions: view.actions.clone(),
            recommendations: view.recommendations.clone(),
        }
    }
}

pub fn write_local_artifacts(
    root: &Path,
    game_id: &str,
    seed: u64,
    final_view: &LocalGameView,
    decision_points: &[LocalDecisionPoint],
) -> Result<LocalArtifactPaths> {
    let dir = root.join(game_id);
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;

    let saved_at = Utc::now().to_rfc3339();
    let replay = LocalReplayArtifact {
        schema_version: LOCAL_ARTIFACT_SCHEMA_VERSION,
        game_id: game_id.to_string(),
        source: final_view.source.clone(),
        seed,
        final_phase_label: final_view.phase_label.clone(),
        final_round: final_view.round.clone(),
        final_players: final_view.players.clone(),
        final_self_hand_tiles: final_view.self_hand_tiles.clone(),
        dora_indicators: final_view.dora_indicators.clone(),
        decision_count: decision_points.len(),
        saved_at: saved_at.clone(),
    };
    let decision_points = LocalDecisionPointArtifact {
        schema_version: LOCAL_ARTIFACT_SCHEMA_VERSION,
        game_id: game_id.to_string(),
        source: final_view.source.clone(),
        decision_points: decision_points.to_vec(),
        saved_at,
    };

    let replay_path = dir.join("replay.json");
    let decision_points_path = dir.join("decision-points.json");
    write_pretty_json(&replay_path, &replay)?;
    write_pretty_json(&decision_points_path, &decision_points)?;

    Ok(LocalArtifactPaths {
        replay_path,
        decision_points_path,
    })
}

fn write_pretty_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let body = serde_json::to_vec_pretty(value).context("serialize local artifact")?;
    std::fs::write(path, body).with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_game::host::LocalGameHost;
    use serde_json::Value;
    use std::collections::BTreeSet;
    use tempfile::TempDir;

    const FORBIDDEN_KEYS: [&str; 7] = [
        "drawTiles",
        "wall",
        "hidden",
        "hidden_state",
        "hands",
        "hiddenTiles",
        "wallTiles",
    ];

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
    fn replay_artifact_serializes_visible_terminal_summary() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());
        session.force_short_draw_pool_for_test(vec!["1p".into(), "2p".into(), "3p".into()]);
        let view = session.view();
        let point = LocalDecisionPoint::from_view(0, &view, &view.actions[0]);
        let ended = session.submit_action(view.actions[0].id).unwrap();
        let tmp = TempDir::new().unwrap();

        let paths =
            write_local_artifacts(tmp.path(), &session.game_id, session.seed, &ended, &[point])
                .unwrap();

        let value: Value =
            serde_json::from_slice(&std::fs::read(paths.replay_path).unwrap()).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["gameId"], "local-test");
        assert_eq!(value["finalPhaseLabel"], "Exhaustive draw");
        assert_eq!(value["decisionCount"], 1);

        let mut keys = BTreeSet::new();
        collect_keys(&value, &mut keys);
        for forbidden in FORBIDDEN_KEYS {
            assert!(
                !keys.contains(forbidden),
                "forbidden key leaked: {forbidden}"
            );
        }
    }

    #[test]
    fn decision_point_artifact_serializes_visible_choices_only() {
        let session = LocalGameHost::new(1).start_session("local-test".into());
        let view = session.view();
        let point = LocalDecisionPoint::from_view(0, &view, &view.actions[0]);
        let tmp = TempDir::new().unwrap();

        let paths =
            write_local_artifacts(tmp.path(), &session.game_id, session.seed, &view, &[point])
                .unwrap();

        let value: Value =
            serde_json::from_slice(&std::fs::read(paths.decision_points_path).unwrap()).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["decisionPoints"].as_array().unwrap().len(), 1);
        assert_eq!(value["decisionPoints"][0]["actionType"], "discard");
        assert!(value["decisionPoints"][0]["actions"]
            .as_array()
            .is_some_and(|actions| !actions.is_empty()));

        let mut keys = BTreeSet::new();
        collect_keys(&value, &mut keys);
        for forbidden in FORBIDDEN_KEYS {
            assert!(
                !keys.contains(forbidden),
                "forbidden key leaked: {forbidden}"
            );
        }
    }
}
