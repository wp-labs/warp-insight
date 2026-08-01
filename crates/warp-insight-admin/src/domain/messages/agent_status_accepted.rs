// @moju generated
// @moju hash=ed0b445234fa1dee

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "response", domain = "Reporting", module = "Reporting.Protocol")]
pub struct AgentStatusAccepted {
    pub snapshot: crate::domain::types::RuntimeHealthSnapshot,
}
