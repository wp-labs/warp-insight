// @moju generated
// @moju hash=002fd7b00bba6934

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control", module = "Control.Registry")]
pub struct AgentPolicyBinding {
    pub bound_at: crate::domain::types::DateTime,
    pub policy_id: String,
    pub agent_id: String,
    pub policy_version: String,
}
