// @moju generated
// @moju hash=fde998639e35cff6

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
pub struct ExporterSource {
    #[moju(unique)]
    pub source_id: String,
    pub kind: String,
}
