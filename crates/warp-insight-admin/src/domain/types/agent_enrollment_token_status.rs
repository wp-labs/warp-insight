// @moju generated
// @moju hash=29d2bb84737948e5

#[derive(
    Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu,
)]
#[moju(kind = "state", domain = "Control")]
pub enum AgentEnrollmentTokenStatus {
    Active,
    Expired,
    Revoked,
    Exhausted,
}
