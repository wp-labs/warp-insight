// @moju generated
// @moju hash=3176176b86aa633d

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control", module = "Control.AgentCommandControl")]
pub struct AgentControlCommand {
    pub requested_by: String,
    pub command_kind: String,
    pub issued_at: crate::domain::types::DateTime,
    pub target_version: String,
    pub sequence: i64,
    pub expires_at: crate::domain::types::DateTime,
    #[moju(unique)]
    pub command_id: String,
    pub payload: String,
    pub agent_id: String,
}
