use serde::{Deserialize, Serialize};
use serde_json::Value;

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
