// @moju generated
// @moju hash=e15b27921b7a2f6b

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
pub struct PreparedReportOrigin {
    pub state_dir: String,
    pub execution_id: String,
}
