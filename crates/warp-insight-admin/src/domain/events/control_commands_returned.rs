// @moju generated
// @moju hash=8761d16131530fe3

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Control")]
pub struct ControlCommandsReturned {
    pub agent_id: String,
    pub instance_id: String,
    pub messages: String,
    pub next_sequence: String,
    pub returned_at: String,
}
