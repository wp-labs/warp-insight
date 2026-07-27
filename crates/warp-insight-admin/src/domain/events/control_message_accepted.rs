// @moju generated
// @moju hash=9a6d7bba16eb2752

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Control")]
pub struct ControlMessageAccepted {
    pub message_id: String,
    pub agent_id: String,
    pub instance_id: String,
    pub accepted_at: String,
}
