// @moju generated
// @moju hash=552bb414a3656898

#[derive(
    Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu,
)]
#[moju(kind = "state", domain = "Control")]
pub enum AgentEnrollmentTokenValidationStatus {
    Valid,
    HashMismatch,
    Expired,
    Revoked,
    Exhausted,
    EnvironmentMismatch,
    HostNotAllowed,
}
