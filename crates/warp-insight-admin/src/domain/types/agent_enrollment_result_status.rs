// @moju generated
// @moju hash=ab50a295bb1e44bf

#[derive(
    Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu,
)]
#[moju(kind = "state", domain = "Control")]
pub enum AgentEnrollmentResultStatus {
    Accepted,
    Rejected,
    PendingReview,
}
