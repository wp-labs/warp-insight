// @moju generated
// @moju hash=2415f01850f0b5fe

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct ActionPlanAckBuilder {
    pub reason_message: String,
    pub dispatch_id: String,
    pub agent_id: String,
    pub action_id: String,
    pub reason_code: String,
    pub execution_id: String,
    pub plan_digest: String,
    pub received_at: String,
    pub acknowledged_at: String,
    pub instance_id: String,
    pub ack_status: String,
    pub queue_position: String,
}
