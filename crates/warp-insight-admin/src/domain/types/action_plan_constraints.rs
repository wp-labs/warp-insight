// @moju generated
// @moju hash=73408617c9281b26

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct ActionPlanConstraints {
    pub approval_mode: String,
    pub step_timeout_default_ms: String,
    pub reason: String,
    pub required_capabilities: String,
    pub requested_by: String,
    pub risk_level: String,
    pub max_total_duration_ms: String,
    pub approval_ref: String,
    pub execution_profile: String,
}
