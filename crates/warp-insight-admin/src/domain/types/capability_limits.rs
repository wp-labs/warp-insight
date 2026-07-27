// @moju generated
// @moju hash=60aa6a4fb865dc14

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct CapabilityLimits {
    pub max_duration_ms: String,
    pub max_stderr_bytes: String,
    pub max_stdout_bytes: String,
}
