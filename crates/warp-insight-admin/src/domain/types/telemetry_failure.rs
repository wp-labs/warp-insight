// @moju generated
// @moju hash=0abf4c75304819d9

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct TelemetryFailure {
    pub kind: String,
    pub detail: String,
}
