// @moju generated
// @moju hash=d1bd562097f2181c

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "command", domain = "Reporting", module = "Reporting.Protocol")]
pub struct ReportActionResult {
    pub execution_id: String,
    pub kind: String,
    pub agent_id: String,
    pub result_attestation: crate::domain::types::ResultAttestation,
    pub action_id: String,
    pub reported_at: crate::domain::types::DateTime,
    pub final_status: String,
    pub result: String,
    pub dispatch_id: String,
    pub plan_digest: String,
    pub report_attempt: i64,
    pub report_id: String,
    pub api_version: String,
    pub instance_id: String,
}
