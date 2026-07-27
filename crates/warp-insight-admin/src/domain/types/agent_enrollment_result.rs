// @moju generated
// @moju hash=c75502bb722a32ca

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control")]
pub struct AgentEnrollmentResult {
    pub credential_bundle: crate::domain::types::AgentCredentialBundle,
    pub initial_config: crate::domain::types::AgentInitialConfig,
    pub reason_code: String,
    pub instance_id: String,
    pub agent_id: String,
    pub policy_binding: crate::domain::types::AgentPolicyBinding,
    pub status: crate::domain::types::AgentEnrollmentResultStatus,
    pub issued_identity: crate::domain::types::AgentIdentity,
}
