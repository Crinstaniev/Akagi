use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGameSessionHandle {
    pub game_id: String,
    pub view: LocalGameView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGameSubmitActionRequest {
    pub game_id: String,
    pub action_id: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGameView {
    pub schema_version: u32,
    pub source: String,
    pub engine: LocalGameEngineMetadata,
    pub phase_label: String,
    pub notice: String,
    pub round: LocalGameRoundView,
    pub players: Vec<LocalGamePlayerView>,
    pub self_hand_tiles: Vec<String>,
    pub dora_indicators: Vec<String>,
    pub actions: Vec<LocalGameActionView>,
    pub recommendations: Vec<LocalGameRecommendationView>,
    #[serde(default)]
    pub artifact_status: LocalArtifactStatus,
    #[serde(default)]
    pub review_summary: LocalReviewSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGameEngineMetadata {
    pub schema_version: u32,
    pub source: String,
    pub status: String,
    pub capabilities: Vec<String>,
    pub note: String,
    #[serde(default)]
    pub worker: LocalWorkerMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalWorkerMetadata {
    pub schema_version: u32,
    pub configured: bool,
    pub agent_kind: String,
    pub label: String,
    pub timeout_ms: Option<u32>,
}

impl Default for LocalWorkerMetadata {
    fn default() -> Self {
        Self {
            schema_version: 1,
            configured: false,
            agent_kind: "random_agent".into(),
            label: "RandomAgent".into(),
            timeout_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalArtifactStatus {
    pub saved: bool,
    pub replay_path: Option<String>,
    pub decision_points_path: Option<String>,
    pub error_message: Option<String>,
}

impl LocalArtifactStatus {
    pub fn pending() -> Self {
        Self {
            saved: false,
            replay_path: None,
            decision_points_path: None,
            error_message: None,
        }
    }

    pub fn saved(replay_path: String, decision_points_path: String) -> Self {
        Self {
            saved: true,
            replay_path: Some(replay_path),
            decision_points_path: Some(decision_points_path),
            error_message: None,
        }
    }

    pub fn failed(message: String) -> Self {
        Self {
            saved: false,
            replay_path: None,
            decision_points_path: None,
            error_message: Some(message),
        }
    }
}

impl Default for LocalArtifactStatus {
    fn default() -> Self {
        Self::pending()
    }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGameRecommendationView {
    pub rank: u32,
    #[serde(default)]
    pub action_id: Option<u32>,
    #[serde(default)]
    pub tile: Option<String>,
    pub label: String,
    pub source: String,
    pub status: String,
    pub note: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub elapsed_ms: Option<f64>,
    #[serde(default)]
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalReviewSummary {
    pub schema_version: u32,
    pub source: String,
    pub total_decisions: u32,
    pub top1_matches: u32,
    #[serde(default)]
    pub top3_matches: u32,
    pub mismatch_count: u32,
    #[serde(default)]
    pub attention_count: u32,
    #[serde(default)]
    pub fallback_count: u32,
    #[serde(default)]
    pub unavailable_count: u32,
    #[serde(default)]
    pub not_ranked_count: u32,
    pub key_choices: Vec<LocalReviewKeyChoice>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalReviewKeyChoice {
    pub turn_index: u32,
    pub human_action_label: String,
    pub recommended_action_label: Option<String>,
    pub human_tile: Option<String>,
    pub recommended_tile: Option<String>,
    pub category: String,
    #[serde(default)]
    pub reason: Option<String>,
}
