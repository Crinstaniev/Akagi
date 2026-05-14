use super::artifact::LocalDecisionPoint;
use crate::schema::local_game::{LocalReviewKeyChoice, LocalReviewSummary};

pub const LOCAL_REVIEW_SUMMARY_SCHEMA_VERSION: u32 = 1;
pub const LOCAL_REVIEW_SUMMARY_SOURCE: &str = "deterministic_baseline";

pub fn build_local_review_summary(decision_points: &[LocalDecisionPoint]) -> LocalReviewSummary {
    let mut top1_matches = 0_u32;
    let mut key_choices = Vec::new();

    for point in decision_points {
        let recommendation = point.recommendations.first();
        match recommendation.and_then(|item| item.action_id) {
            Some(action_id) if action_id == point.action_id => {
                top1_matches += 1;
            }
            Some(_) => key_choices.push(key_choice(point, "baseline_mismatch")),
            None => key_choices.push(key_choice(point, "unavailable")),
        }
    }

    LocalReviewSummary {
        schema_version: LOCAL_REVIEW_SUMMARY_SCHEMA_VERSION,
        source: LOCAL_REVIEW_SUMMARY_SOURCE.into(),
        total_decisions: decision_points.len() as u32,
        top1_matches,
        mismatch_count: key_choices.len() as u32,
        attention_count: key_choices.len() as u32,
        fallback_count: key_choices
            .iter()
            .filter(|choice| choice.category == "unavailable")
            .count() as u32,
        unavailable_count: key_choices
            .iter()
            .filter(|choice| choice.category == "unavailable")
            .count() as u32,
        not_ranked_count: 0,
        key_choices,
        note: "Deterministic baseline summary only; not a real AI EV review.".into(),
    }
}

fn key_choice(point: &LocalDecisionPoint, category: &str) -> LocalReviewKeyChoice {
    let recommendation = point.recommendations.first();
    LocalReviewKeyChoice {
        turn_index: point.turn_index,
        human_action_label: point.action_label.clone(),
        recommended_action_label: recommendation.map(|item| item.label.clone()),
        human_tile: point.tile.clone(),
        recommended_tile: recommendation.and_then(|item| item.tile.clone()),
        category: category.into(),
        reason: Some(category.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_game::host::LocalGameHost;
    use serde_json::Value;
    use std::collections::BTreeSet;

    fn first_point() -> LocalDecisionPoint {
        let session = LocalGameHost::new(1).start_session("local-test".into());
        let view = session.view();
        LocalDecisionPoint::from_view(0, &view, &view.actions[0])
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

    #[test]
    fn summary_counts_top1_matches() {
        let point = first_point();
        let summary = build_local_review_summary(&[point]);

        assert_eq!(summary.total_decisions, 1);
        assert_eq!(summary.top1_matches, 1);
        assert_eq!(summary.mismatch_count, 0);
        assert!(summary.key_choices.is_empty());
    }

    #[test]
    fn summary_lists_baseline_mismatch_key_choice() {
        let mut point = first_point();
        point.action_id = point.action_id + 1;
        point.action_label = "Discard different".into();

        let summary = build_local_review_summary(&[point]);

        assert_eq!(summary.total_decisions, 1);
        assert_eq!(summary.top1_matches, 0);
        assert_eq!(summary.mismatch_count, 1);
        assert_eq!(summary.key_choices[0].category, "baseline_mismatch");
        assert_eq!(
            summary.key_choices[0].human_action_label,
            "Discard different"
        );
        assert!(summary.key_choices[0].recommended_action_label.is_some());
    }

    #[test]
    fn summary_lists_unavailable_key_choice() {
        let mut point = first_point();
        point.recommendations.clear();

        let summary = build_local_review_summary(&[point]);

        assert_eq!(summary.top1_matches, 0);
        assert_eq!(summary.mismatch_count, 1);
        assert_eq!(summary.key_choices[0].category, "unavailable");
        assert!(summary.key_choices[0].recommended_action_label.is_none());
    }

    #[test]
    fn summary_does_not_expose_hidden_fields() {
        let point = first_point();
        let summary = build_local_review_summary(&[point]);
        let value = serde_json::to_value(summary).unwrap();
        let mut keys = BTreeSet::new();
        collect_keys(&value, &mut keys);

        for forbidden in [
            "drawTiles",
            "wall",
            "hidden",
            "hidden_state",
            "hands",
            "hiddenTiles",
            "wallTiles",
        ] {
            assert!(
                !keys.contains(forbidden),
                "forbidden key leaked: {forbidden}"
            );
        }
    }
}
