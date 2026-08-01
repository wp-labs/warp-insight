// @moju generated
// @moju hash=0c1612003d84d7af

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct StepActionRecord {
    pub started_at: String,
    pub finished_at: String,
    pub error_code: String,
    pub stdout_summary: String,
    pub attempt: String,
    pub op: String,
    pub status: String,
    pub stderr_summary: String,
    pub resource_usage: String,
    pub duration_ms: String,
    #[moju(unique)]
    pub step_id: String,
}
