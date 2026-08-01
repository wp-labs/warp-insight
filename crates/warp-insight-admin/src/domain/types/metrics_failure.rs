// @moju generated
// @moju hash=475960500263201f

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Health")]
pub struct MetricsFailure {
    pub detail: String,
    pub kind: String,
}
