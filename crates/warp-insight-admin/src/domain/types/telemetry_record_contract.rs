// @moju generated
// @moju hash=be7967fcf278dc28

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct TelemetryRecordContract {
    pub kind: String,
    #[moju(unique)]
    pub record_id: String,
    pub agent_id: String,
    pub instance_id: String,
    pub collected_at: String,
    pub api_version: String,
}
