use super::artifact::{write_local_artifacts, LocalDecisionPoint};
use super::review::build_local_review_summary;
use crate::schema::local_game::{
    LocalArtifactStatus, LocalGameActionView, LocalGamePlayerView, LocalGameRecommendationView,
    LocalGameRoundView, LocalGameView,
};
use serde_json::json;
use std::path::Path;

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

    pub fn persist_artifacts(&mut self, root: &Path) {
        if !self.state.ended || self.state.artifact_status.saved {
            return;
        }
        let view = self.view();
        match write_local_artifacts(
            root,
            &self.game_id,
            self.seed,
            &view,
            &self.state.decision_points,
        ) {
            Ok(paths) => {
                self.state.artifact_status = LocalArtifactStatus::saved(
                    paths.replay_path.to_string_lossy().into_owned(),
                    paths.decision_points_path.to_string_lossy().into_owned(),
                );
            }
            Err(error) => {
                self.state.artifact_status = LocalArtifactStatus::failed(error.to_string());
            }
        }
    }

    #[cfg(test)]
    pub fn decision_point_count_for_test(&self) -> usize {
        self.state.decision_points.len()
    }

    #[cfg(test)]
    pub fn force_short_draw_pool_for_test(&mut self, draw_tiles: Vec<String>) {
        self.state.remaining_tiles = draw_tiles.len() as u32;
        self.state.draw_tiles = draw_tiles;
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
    draw_tiles: Vec<String>,
    dora_indicator: String,
    remaining_tiles: u32,
    turn_index: u32,
    actions_enabled: bool,
    ended: bool,
    decision_points: Vec<LocalDecisionPoint>,
    artifact_status: LocalArtifactStatus,
}

impl LocalGameSessionState {
    fn from_seed(seed: u64) -> Self {
        let mut wall = tile_wall();
        let offset = (seed as usize) % wall.len();
        wall.rotate_left(offset);

        let self_hand_tiles = wall.iter().take(14).cloned().collect();
        let draw_tiles = wall.iter().skip(14).take(70).cloned().collect::<Vec<_>>();
        let dora_indicator = wall.get(53).cloned().unwrap_or_else(|| "5m".into());
        let remaining_tiles = draw_tiles.len() as u32;

        Self {
            self_hand_tiles,
            player_river_tiles: vec![vec![], vec![], vec![], vec![]],
            draw_tiles,
            dora_indicator,
            remaining_tiles,
            turn_index: 0,
            actions_enabled: true,
            ended: false,
            decision_points: vec![],
            artifact_status: LocalArtifactStatus::pending(),
        }
    }

    fn view(&self) -> LocalGameView {
        let actions = self.discard_actions();
        let recommendation = recommendation_from_first_action(&actions);
        LocalGameView {
            schema_version: 1,
            source: "local_game_host".into(),
            phase_label: if self.ended {
                "Exhaustive draw".into()
            } else if self.turn_index == 0 {
                "Initial local hand".into()
            } else {
                "Waiting for your action".into()
            },
            notice: if self.ended {
                "The deterministic local lifecycle reached exhaustive draw. Scoring is not wired yet."
                    .into()
            } else if self.turn_index == 0 {
                "Local game session view. Submit a discard to update this session.".into()
            } else {
                "Deterministic local lifecycle advanced to your next draw. CoachWorker is not wired yet."
                    .into()
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
            artifact_status: self.artifact_status.clone(),
            review_summary: build_local_review_summary(&self.decision_points),
        }
    }

    fn submit_action(&mut self, action_id: u32) -> Result<(), String> {
        if self.ended {
            return Err("local game session already ended".into());
        }
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

        let before_view = self.view();
        self.decision_points.push(LocalDecisionPoint::from_view(
            self.turn_index,
            &before_view,
            action,
        ));
        let tile = self.self_hand_tiles.remove(index);
        self.player_river_tiles[0].push(tile);
        self.advance_lifecycle();
        self.turn_index += 1;
        Ok(())
    }

    fn advance_lifecycle(&mut self) {
        for seat in 1..=3 {
            let Some(tile) = self.draw_tile() else {
                self.end_exhaustive_draw();
                return;
            };
            self.player_river_tiles[seat].push(tile);
        }

        let Some(tile) = self.draw_tile() else {
            self.end_exhaustive_draw();
            return;
        };
        self.self_hand_tiles.push(tile);
    }

    fn draw_tile(&mut self) -> Option<String> {
        if self.remaining_tiles == 0 {
            return None;
        }
        let tile = self.draw_tiles.pop()?;
        self.remaining_tiles = self.remaining_tiles.saturating_sub(1);
        Some(tile)
    }

    fn end_exhaustive_draw(&mut self) {
        self.ended = true;
        self.actions_enabled = false;
        self.remaining_tiles = 0;
    }

    fn discard_actions(&self) -> Vec<LocalGameActionView> {
        if self.ended {
            return vec![];
        }
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
        note: if action.is_some() {
            "Deterministic placeholder until CoachWorker is wired.".into()
        } else {
            "The local hand ended in exhaustive draw.".into()
        },
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

        assert_eq!(after.self_hand_tiles.len(), before.self_hand_tiles.len());
        let self_player = after.players.iter().find(|player| player.is_self).unwrap();
        assert_eq!(self_player.river_tiles, vec![discarded]);
        assert_eq!(after.actions.len(), before.actions.len());
        assert_eq!(after.phase_label, "Waiting for your action");
        assert!(after.notice.contains("next draw"));
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
    fn repeated_submit_keeps_self_hand_sustainable() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());

        for _ in 0..3 {
            let before = session.view();
            let action_id = before.actions[0].id;
            let after = session.submit_action(action_id).unwrap();

            assert_eq!(after.self_hand_tiles.len(), 14);
            assert!(after.actions.iter().any(|action| action.enabled));
        }
    }

    #[test]
    fn repeated_submit_grows_all_rivers() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());

        let first = session.submit_action(1).unwrap();
        let second = session.submit_action(first.actions[0].id).unwrap();

        for seat in 0..=3 {
            assert_eq!(river_len(&second, seat), 2);
        }
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
    fn submit_enters_exhaustive_draw_when_draw_pool_runs_out() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());
        session.state.draw_tiles = vec!["1p".into(), "2p".into(), "3p".into()];
        session.state.remaining_tiles = session.state.draw_tiles.len() as u32;
        let before = session.view();

        let after = session.submit_action(before.actions[0].id).unwrap();

        assert_eq!(after.phase_label, "Exhaustive draw");
        assert_eq!(after.round.remaining_tiles, 0);
        assert!(after.actions.is_empty());
        assert_eq!(after.recommendations[0].status, "unavailable");
        assert!(after.recommendations[0].action_id.is_none());
    }

    #[test]
    fn terminal_submit_returns_error_and_preserves_view() {
        let mut session = LocalGameHost::new(1).start_session("local-test".into());
        session.state.draw_tiles = vec!["1p".into(), "2p".into(), "3p".into()];
        session.state.remaining_tiles = session.state.draw_tiles.len() as u32;
        let action_id = session.view().actions[0].id;
        let ended = session.submit_action(action_id).unwrap();

        let error = session.submit_action(action_id).unwrap_err();

        assert!(error.contains("already ended"));
        assert_eq!(session.view(), ended);
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
