// @moju generated
// @moju hash=c1dd2de3c508b773

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct ReportingRequest {
    pub instance_id: String,
    pub agent_id: String,
    pub state_dir: String,
    pub execution_id: String,
}
