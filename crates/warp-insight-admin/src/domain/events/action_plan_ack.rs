// @moju generated
// @moju hash=fc2e0eec800f2d2f

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Reporting")]
pub struct ActionPlanAck {
    pub api_version: String,
    pub kind: String,
    pub dispatch_id: String,
    pub action_id: String,
    pub plan_digest: String,
    pub agent_id: String,
    pub instance_id: String,
    pub execution_id: String,
    pub ack_status: String,
    pub reason_code: String,
    pub reason_message: String,
    pub queue_position: String,
    pub received_at: String,
    pub acknowledged_at: String,
}
