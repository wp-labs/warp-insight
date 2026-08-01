// @moju generated
// @moju hash=7ec1773aaebcdc92

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "command", domain = "Reporting", module = "Reporting.Protocol")]
pub struct AgentHello {
    pub instance_id: String,
    pub version: String,
    pub agent_id: String,
}
