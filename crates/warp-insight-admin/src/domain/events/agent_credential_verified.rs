// @moju generated
// @moju hash=4c52643461d17585

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Control")]
pub struct AgentCredentialVerified {
    pub agent_id: String,
    pub instance_id: String,
    pub verified_at: String,
}
