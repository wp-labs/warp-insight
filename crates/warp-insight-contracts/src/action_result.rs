//! `ActionResult` contract types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::API_VERSION_V1;

pub const ACTION_RESULT_KIND: &str = "action_result";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionResultContract {
    pub api_version: String,
    pub kind: String,
    pub action_id: String,
    pub execution_id: String,
    pub request_id: Option<String>,
    pub final_status: FinalStatus,
    pub exit_reason: Option<String>,
    pub step_records: Vec<StepActionRecord>,
    pub outputs: ActionOutputs,
    pub resource_usage: Option<ExecutionResourceUsage>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

impl ActionResultContract {
    pub fn new(action_id: String, execution_id: String, final_status: FinalStatus) -> Self {
        Self {
            api_version: API_VERSION_V1.to_string(),
            kind: ACTION_RESULT_KIND.to_string(),
            action_id,
            execution_id,
            request_id: None,
            final_status,
            exit_reason: None,
            step_records: Vec::new(),
            outputs: ActionOutputs::default(),
            resource_usage: None,
            started_at: None,
            finished_at: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinalStatus {
    #[serde(rename = "succeeded")]
    Succeeded,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "timed_out")]
    TimedOut,
    #[serde(rename = "rejected")]
    Rejected,
}

impl FinalStatus {
    pub fn as_state_name(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepActionRecord {
    pub step_id: String,
    pub attempt: u32,
    pub op: Option<String>,
    pub status: StepStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub error_code: Option<String>,
    pub stdout_summary: Option<String>,
    pub stderr_summary: Option<String>,
    pub resource_usage: Option<ExecutionResourceUsage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "succeeded")]
    Succeeded,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "timed_out")]
    TimedOut,
    #[serde(rename = "skipped")]
    Skipped,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionOutputs {
    #[serde(default)]
    pub items: Vec<ActionOutputItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionOutputItem {
    pub name: String,
    pub value: Value,
    pub redacted: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionResourceUsage {
    pub max_rss_bytes: Option<u64>,
    pub cpu_time_ms: Option<u64>,
    pub stdout_bytes: Option<u64>,
    pub stderr_bytes: Option<u64>,
}
