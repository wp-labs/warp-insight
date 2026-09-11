// @jumo generated
// @jumo hash=7ec1773aaebcdc92

#[derive(Debug, Clone, Copy, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWorkState {
    Paused,
    Resumed,
}

/// 工作状态变化（非告警、非失败）：暂停/恢复各上报一次。
#[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
pub struct AgentWorkStateChange {
    pub input_id: String,
    pub state: AgentWorkState,
    pub reason: String,
    pub at: String,
}

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "message",
    role = "command",
    domain = "Reporting",
    module = "Reporting.Protocol"
)]
pub struct AgentHello {
    pub instance_id: String,
    pub version: String,
    pub agent_id: String,
    #[serde(default)]
    pub memory_bytes: Option<i64>,
    #[serde(default)]
    pub cpu_percent: Option<f64>,
    #[serde(default)]
    pub admin_latency_ms: Option<i64>,
    #[serde(default)]
    pub work_state_changes: Option<Vec<AgentWorkStateChange>>,
}
