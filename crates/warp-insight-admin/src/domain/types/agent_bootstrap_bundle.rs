// @moju generated
// @moju hash=5a7809976f6c209f

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control")]
pub struct AgentBootstrapBundle {
    pub expires_at: crate::domain::types::DateTime,
    pub environment_id: String,
    pub control_endpoint: String,
    pub tenant_id: String,
    #[moju(unique)]
    pub bundle_id: String,
    pub install_script_url: String,
    pub agent_package_url: String,
    pub trust_bundle: String,
}
