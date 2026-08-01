// @moju generated
// @moju hash=e6f42e905f355824

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Health")]
pub struct MetricsHealthSnapshot {
    pub sample_count: i64,
    pub target_count: i64,
    pub failure_count: i64,
    pub active: bool,
}
