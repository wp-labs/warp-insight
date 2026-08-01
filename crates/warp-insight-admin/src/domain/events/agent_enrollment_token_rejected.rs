// @moju generated
// @moju hash=d8862d2eaef92bea

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Control", module = "Control.Enrollment")]
pub struct AgentEnrollmentTokenRejected {
    pub token_id: String,
    pub node_id: String,
    pub reason_code: String,
    pub rejected_at: String,
}
