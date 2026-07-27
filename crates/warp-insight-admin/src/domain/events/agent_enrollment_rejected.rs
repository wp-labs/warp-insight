// @moju generated
// @moju hash=0941aef4756feb6b

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Control")]
pub struct AgentEnrollmentRejected {
    pub token_id: String,
    pub node_id: String,
    pub reason_code: String,
    pub rejected_at: String,
}
