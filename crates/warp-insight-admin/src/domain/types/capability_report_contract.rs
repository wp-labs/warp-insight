// @moju generated
// @moju hash=a280532d41ee8d3e

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct CapabilityReportContract {
    pub schema_version: String,
    pub version: String,
    pub sections: String,
    pub agent_id: String,
    pub instance_id: String,
    pub generated_at: String,
}
