// @moju generated
// @moju hash=605bb21e8630f7ba

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct MetricsCapabilities {
    pub supported: String,
    pub collection_kinds: String,
}
