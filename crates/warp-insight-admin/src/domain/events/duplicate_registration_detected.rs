// @moju generated
// @moju hash=49d4bd67c642a1a3

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Control")]
pub struct DuplicateRegistrationDetected {
    pub node_id: String,
    pub existing_agent_id: String,
    pub candidate_instance_id: String,
    pub action: String,
    pub detected_at: String,
}
