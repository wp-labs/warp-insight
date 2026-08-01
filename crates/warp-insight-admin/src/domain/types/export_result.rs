// @moju generated
// @moju hash=8bfac24d3ba64827

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
pub struct ExportResult {
    pub items: String,
    pub errors: String,
    pub kind: String,
}
