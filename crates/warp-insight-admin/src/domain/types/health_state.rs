// @moju generated
// @moju hash=e39a0b357a3492b2

#[derive(
    Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu,
)]
#[moju(kind = "state", domain = "Reporting")]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
}
