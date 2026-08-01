// @moju generated
// @moju hash=8ff6fa619801f698

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Protocol")]
pub struct ActionResultReceipt {
    pub agent_id: String,
    pub report_id: String,
    pub accepted_at: crate::domain::types::DateTime,
}
