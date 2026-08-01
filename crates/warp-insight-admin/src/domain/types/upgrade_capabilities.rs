// @moju generated
// @moju hash=39faaf0d84182eff

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct UpgradeCapabilities {
    pub stage_kinds: String,
    pub supported: String,
}
