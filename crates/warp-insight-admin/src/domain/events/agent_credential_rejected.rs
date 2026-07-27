// @moju generated
// @moju hash=a1b7cd54c5a25543

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Control")]
pub struct AgentCredentialRejected {
    pub agent_id: String,
    pub instance_id: String,
    pub reason_code: String,
    pub rejected_at: String,
}
