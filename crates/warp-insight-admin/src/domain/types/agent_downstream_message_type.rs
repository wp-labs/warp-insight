// @moju generated
// @moju hash=20efa995a3fe7976

#[derive(
    Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu,
)]
#[moju(kind = "state", domain = "Control")]
pub enum AgentDownstreamMessageType {
    EnrollmentResult,
    ControlCommands,
    PolicyRefreshHint,
    IdentityRotationHint,
}
