// @moju generated
// @moju hash=e0a5eb5309c65c1f

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct LogsCapabilities {
    pub output_kinds: String,
    pub supported: String,
    pub input_kinds: String,
}
