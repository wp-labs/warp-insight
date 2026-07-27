// @moju generated
// @moju hash=ecc04201075c3e3c

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct ActionPlanMeta {
    pub plan_version: String,
    pub request_id: String,
    pub environment_id: String,
    pub expires_at: String,
    pub tenant_id: String,
    pub compiled_at: String,
    #[moju(unique)]
    pub action_id: String,
    pub template_id: String,
}
