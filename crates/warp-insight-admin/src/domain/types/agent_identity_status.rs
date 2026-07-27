// @moju generated
// @moju hash=df19454bec80dade

#[derive(
    Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu,
)]
#[moju(kind = "state", domain = "Control")]
pub enum AgentIdentityStatus {
    Active,
    Revoked,
    Expired,
    RenewalRequired,
}
