// @moju generated
// @moju hash=776b3eddfe95851e

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control")]
pub struct AgentEnrollmentToken {
    pub revoked_at: crate::domain::types::DateTime,
    pub allowed_node_selector: String,
    pub tenant_id: String,
    pub environment_id: String,
    pub issued_at: crate::domain::types::DateTime,
    pub used_count: i64,
    pub issued_by: String,
    #[moju(unique)]
    pub token_id: String,
    pub max_uses: i64,
    pub expires_at: crate::domain::types::DateTime,
    pub token_hash: String,
    pub status: crate::domain::types::AgentEnrollmentTokenStatus,
}
