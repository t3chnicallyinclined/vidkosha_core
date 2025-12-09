use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: Option<String>,
    pub agent_name: String,
    pub topic: String,
    pub project: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub summary: String,
    pub full_content: String,
    pub confidence: f32,
    #[serde(default)]
    pub open_questions: Vec<String>,
    #[serde(default)]
    pub perspectives: Vec<PerspectiveView>,
    #[serde(default)]
    pub messages: Vec<MessageRecord>,
    #[serde(default)]
    pub artifacts: Vec<ArtifactRef>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCallRecord>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerspectiveView {
    pub role: String,
    pub summary: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risks: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decisions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MemoryWriteRequest {
    pub record: MemoryRecord,
}

#[derive(Debug, Clone)]
pub struct MemoryWriteResponse {
    pub memory_id: String,
}

#[derive(Debug, Clone)]
pub struct MemoryDeleteRequest {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageRecord {
    #[serde(default)]
    pub message_id: Option<String>,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArtifactRef {
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolCallRecord {
    #[serde(default)]
    pub tool_call_id: Option<String>,
    pub tool_name: String,
    pub args_json: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_summary: Option<String>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    pub request_id: String,
    pub agent_name: String,
    pub operator_id: Option<String>,
    pub specialist_agent_id: Option<String>,
    pub tool_name: String,
    pub tokens_consumed: u64,
    pub model: Option<String>,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PayoutSplit {
    pub operator_share: f64,
    pub treasury_share: f64,
    pub evaluator_share: f64,
    #[serde(default)]
    pub insurance_share: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutEvent {
    pub request_id: String,
    pub operator_id: String,
    pub specialist_agent_id: String,
    pub tokens_settled: u64,
    pub total_cost: f64,
    pub split: PayoutSplit,
    pub rating: Option<f32>,
    pub evaluator_notes: Option<String>,
    pub model_version: Option<String>,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryFilters {
    pub agent_name: Option<String>,
    pub topic: Option<String>,
    pub project: Option<String>,
    pub conversation_id: Option<String>,
    pub since: Option<DateTime<Utc>>,
}

impl MemoryFilters {
    pub fn matches(&self, record: &MemoryRecord) -> bool {
        self.agent_name
            .as_ref()
            .is_none_or(|needle| record.agent_name == *needle)
            && self
                .topic
                .as_ref()
                .is_none_or(|needle| record.topic == *needle)
            && self
                .project
                .as_ref()
                .is_none_or(|needle| record.project.as_deref() == Some(needle.as_str()))
            && self
                .conversation_id
                .as_ref()
                .is_none_or(|needle| record.conversation_id.as_deref() == Some(needle.as_str()))
            && self
                .since
                .as_ref()
                .is_none_or(|since| record.timestamp >= *since)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQuery {
    pub query: String,
    pub filters: MemoryFilters,
    pub limit: usize,
}

impl MemoryQuery {
    pub fn limit(&self) -> usize {
        self.limit.clamp(1, 50)
    }
}

#[derive(Debug, Clone)]
pub enum MemoryRequest {
    Write(MemoryWriteRequest),
    #[allow(dead_code)]
    Retrieve(MemoryQuery),
    #[allow(dead_code)]
    Delete(MemoryDeleteRequest),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MemoryResponse {
    pub notes: String,
    #[allow(dead_code)]
    pub records: Vec<MemoryRecord>,
    pub memory_ids: Vec<String>,
}
