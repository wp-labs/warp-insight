// @moju generated
// @moju hash=762a98664f991f1c

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control", module = "Control.AgentCommandControl")]
pub struct DispatchReceipt {
    pub created_at: crate::domain::types::DateTime,
    pub command_id: String,
    pub dispatch_id: String,
    pub status: String,
    pub agent_id: String,
}
