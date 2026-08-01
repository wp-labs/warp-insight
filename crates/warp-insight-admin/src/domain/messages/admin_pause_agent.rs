// @moju generated
// @moju hash=3713107a9dcda46f

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "command", domain = "Control", module = "Control.UserFacingInterface")]
pub struct AdminPauseAgent {
    pub agent_id: String,
    pub requested_by: String,
}
