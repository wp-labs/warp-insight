// @moju generated
// @moju hash=85bf9c3b74cb8b68

#[derive(
    Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu,
)]
#[moju(kind = "state", domain = "Control")]
pub enum AgentCredentialVerificationStatus {
    Verified,
    UnknownAgent,
    CredentialMissing,
    CredentialExpired,
    CredentialRevoked,
    CredentialMismatch,
}
