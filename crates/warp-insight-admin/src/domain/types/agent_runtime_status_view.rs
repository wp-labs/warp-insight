// @moju generated
// @moju hash=b505b229ad05d5d4

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control")]
pub struct AgentRuntimeStatusView {
    pub version: String,
    pub health: String,
    pub instance_id: String,
    pub agent_id: String,
    pub status: String,
    pub last_seen_at: crate::domain::types::DateTime,
}
