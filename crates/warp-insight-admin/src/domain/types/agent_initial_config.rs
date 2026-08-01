// @moju generated
// @moju hash=b046700e50f48336

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control", module = "Control.Registry")]
pub struct AgentInitialConfig {
    pub gateway_endpoint: String,
    pub schema_version: String,
    pub telemetry_output: String,
    pub policy_version: String,
    pub mode: String,
}
