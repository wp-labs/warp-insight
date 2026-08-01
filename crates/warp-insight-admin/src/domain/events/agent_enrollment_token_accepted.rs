// @moju generated
// @moju hash=49d79d5f2eecbd47

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Control", module = "Control.Enrollment")]
pub struct AgentEnrollmentTokenAccepted {
    pub token_id: String,
    pub tenant_id: String,
    pub environment_id: String,
    pub node_id: String,
    pub accepted_at: String,
}
