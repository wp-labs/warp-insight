// @moju generated
// @moju hash=b3c4d43dade74294

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct ActionResultContract {
    pub step_records: String,
    pub execution_id: String,
    pub exit_reason: String,
    pub started_at: String,
    pub outputs: String,
    pub api_version: String,
    pub resource_usage: String,
    pub action_id: String,
    pub kind: String,
    pub request_id: String,
    pub finished_at: String,
    pub final_status: String,
}
