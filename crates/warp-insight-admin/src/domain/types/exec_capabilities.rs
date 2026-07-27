// @moju generated
// @moju hash=47a438ed8b14e723

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct ExecCapabilities {
    pub cancel_supported: String,
    pub max_concurrent: String,
    pub supported: String,
}
