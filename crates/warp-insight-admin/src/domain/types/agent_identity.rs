// @moju generated
// @moju hash=2ddd5bd704f3783b

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control", module = "Control.Identity")]
pub struct AgentIdentity {
    pub tenant_id: String,
    pub expires_at: crate::domain::types::DateTime,
    pub status: crate::domain::types::AgentIdentityStatus,
    pub instance_id: String,
    pub environment_id: String,
    #[moju(unique)]
    pub agent_id: String,
    pub issued_at: crate::domain::types::DateTime,
    pub node_id: String,
}
