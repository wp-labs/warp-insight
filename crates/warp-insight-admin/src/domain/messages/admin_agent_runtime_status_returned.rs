// @moju generated
// @moju hash=4186b433ea3b839c

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "response", domain = "Control", module = "Control.UserFacingInterface")]
pub struct AdminAgentRuntimeStatusReturned {
    pub status: crate::domain::types::AgentRuntimeStatusView,
}
