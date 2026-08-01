// @moju generated
// @moju hash=14428160952fb811

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "response", domain = "Control", module = "Control.AgentFacingInterface")]
pub struct AgentControlCommandsReturned {
    pub messages: Vec<crate::domain::types::AgentControlCommand>,
    pub next_sequence: i64,
    pub agent_id: String,
    pub returned_at: crate::domain::types::DateTime,
    pub instance_id: String,
}
