// @moju generated
// @moju hash=ec6451692d1963f3

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control")]
pub struct AgentInstance {
    pub started_at: crate::domain::types::DateTime,
    pub last_seen_at: crate::domain::types::DateTime,
    pub agent_id: String,
    #[moju(unique)]
    pub instance_id: String,
    pub version: String,
    pub boot_id: String,
}
