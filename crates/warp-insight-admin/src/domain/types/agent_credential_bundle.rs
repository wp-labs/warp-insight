// @moju generated
// @moju hash=cec6e68eee826022

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control", module = "Control.Identity")]
pub struct AgentCredentialBundle {
    pub not_before: crate::domain::types::DateTime,
    pub not_after: crate::domain::types::DateTime,
    pub certificate: String,
    pub auth_scheme: String,
    pub bearer_token: String,
    pub ca_bundle: String,
    pub instance_id: String,
    pub issued_at: crate::domain::types::DateTime,
    #[moju(unique)]
    pub credential_id: String,
    pub private_key_ref: String,
    pub agent_id: String,
}
