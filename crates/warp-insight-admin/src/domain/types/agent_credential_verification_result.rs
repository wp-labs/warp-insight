// @moju generated
// @moju hash=cfb5641c937ad6f8

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control")]
pub struct AgentCredentialVerificationResult {
    pub agent_id: String,
    pub instance_id: String,
    pub reason_code: String,
    pub status: crate::domain::types::AgentCredentialVerificationStatus,
    pub verified_at: crate::domain::types::DateTime,
}
