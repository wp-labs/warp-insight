// @moju generated
// @moju hash=29287a669303a0ed

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct ExecutionHistoryRecord {
    pub plan_digest: String,
    pub action_id: String,
    pub final_status: String,
    pub finished_at: String,
    pub execution_id: String,
}
