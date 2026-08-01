// @moju generated
// @moju hash=3c0d307493621113

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "command", domain = "Control", module = "Control.UserFacingInterface")]
pub struct AdminUpgradeAgent {
    pub requested_by: String,
    pub agent_id: String,
    pub target_version: String,
}
