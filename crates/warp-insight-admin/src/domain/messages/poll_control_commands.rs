// @moju generated
// @moju hash=d829890b1e8302c4

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "command", domain = "Control")]
pub struct PollControlCommands {
    pub requested_at: crate::domain::types::DateTime,
    pub last_seen_sequence: i64,
    pub wait_ms: i64,
    pub agent_id: String,
    pub instance_id: String,
}
