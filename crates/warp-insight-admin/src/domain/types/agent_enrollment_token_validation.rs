// @moju generated
// @moju hash=c04fc3172036cc4c

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control", module = "Control.Enrollment")]
pub struct AgentEnrollmentTokenValidation {
    pub host_profile: crate::domain::types::AgentHostProfile,
    pub validated_at: crate::domain::types::DateTime,
    pub status: crate::domain::types::AgentEnrollmentTokenValidationStatus,
    pub tenant_id: String,
    pub environment_id: String,
    pub token_id: String,
    pub reason_code: String,
}
