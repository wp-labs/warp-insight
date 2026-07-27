// @moju generated
// @moju hash=c3988499d829088d

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control")]
pub struct AgentControlAuthProfile {
    pub enrollment_token_rejected: bool,
    pub mtls_client_certificate_allowed: bool,
    pub agent_credential_required: bool,
    pub server_tls_required: bool,
}
