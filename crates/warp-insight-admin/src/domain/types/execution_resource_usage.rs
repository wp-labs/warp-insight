// @moju generated
// @moju hash=8daba08b85e0da35

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct ExecutionResourceUsage {
    pub stderr_bytes: String,
    pub cpu_time_ms: String,
    pub stdout_bytes: String,
    pub max_rss_bytes: String,
}
