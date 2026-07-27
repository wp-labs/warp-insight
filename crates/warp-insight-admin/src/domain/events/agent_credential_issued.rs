// @moju generated
// @moju hash=b722159e25388a6a

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Control")]
pub struct AgentCredentialIssued {
    pub agent_id: String,
    pub instance_id: String,
    pub credential: String,
    pub issued_at: String,
}
