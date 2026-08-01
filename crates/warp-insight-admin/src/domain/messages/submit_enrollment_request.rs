// @moju generated
// @moju hash=f89352363bbea22d

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "command", domain = "Control", module = "Control.AgentFacingInterface")]
pub struct SubmitEnrollmentRequest {
    pub capability_summary: String,
    pub token: String,
    pub requested_at: crate::domain::types::DateTime,
    pub host_profile: crate::domain::types::AgentHostProfile,
    pub credential_request: String,
}
