use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LocalGameConfig {
    /// JSONL AI worker command used by Local Table backend sessions.
    /// Empty string means "not configured".
    pub ai_worker_cmd: String,
    /// Optional per-action worker timeout. `None` lets the Python backend
    /// keep its default timeout.
    pub ai_worker_timeout_ms: Option<u32>,
    /// Last Mortal model directory used by the Settings command generator.
    /// This is UI input only; the backend session consumes `ai_worker_cmd`.
    pub mortal_model_dir: String,
}
