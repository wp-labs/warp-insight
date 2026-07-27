// @moju generated
// @moju hash=08f280facbcbf272

#[derive(
    Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu,
)]
#[moju(kind = "state", domain = "Control")]
pub enum AgentUpstreamMessageType {
    EnrollmentRequest,
    StatusReport,
    CommandPoll,
    ActionResult,
}
