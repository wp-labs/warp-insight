// @moju generated
// @moju hash=935e9782936adab0

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct CapabilityReportSections {
    pub exec: String,
    pub logs: String,
    pub upgrade: String,
    pub metrics: String,
    pub limits: String,
}
