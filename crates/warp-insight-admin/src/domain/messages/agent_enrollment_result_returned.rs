// @moju generated
// @moju hash=bce5b039157990bd

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "response", domain = "Control")]
pub struct AgentEnrollmentResultReturned {
    pub result: crate::domain::types::AgentEnrollmentResult,
}
