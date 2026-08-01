// @moju generated
// @moju hash=3b97dd40e692bd73

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Control", module = "Control.Identity")]
pub struct AgentCredentialRevoked {
    pub agent_id: String,
    pub instance_id: String,
    pub reason_code: String,
    pub revoked_at: String,
}
