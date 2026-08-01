// @moju generated
// @moju hash=8ef7b8ca57bc3ab0

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct ActionPlanTarget {
    pub selectors: String,
    pub arch: String,
    pub host_name: String,
    pub agent_id: String,
    pub node_id: String,
    pub instance_id: String,
    pub platform: String,
}
