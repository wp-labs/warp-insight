// @moju generated
// @moju hash=c55e816ea5aff35e

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "command", domain = "Control", module = "Control.UserFacingInterface")]
pub struct AdminShowAgentRuntimeStatus {
    pub requested_by: String,
    pub agent_id: String,
}
