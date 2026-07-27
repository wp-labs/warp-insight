// @moju generated
// @moju hash=19ba200dc55576ee

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Control")]
pub struct ControlLongPollTimedOut {
    pub agent_id: String,
    pub instance_id: String,
    pub wait_ms: String,
    pub timed_out_at: String,
}
