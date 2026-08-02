// @moju generated
// @moju hash=7ec1773aaebcdc92

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "command", domain = "Reporting", module = "Reporting.Protocol")]
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
}
