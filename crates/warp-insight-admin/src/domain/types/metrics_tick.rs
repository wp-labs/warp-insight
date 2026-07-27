// @moju generated
// @moju hash=8b378da3c456e28d

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct MetricsTick {
    pub failures: String,
    pub health_snapshot: String,
}
