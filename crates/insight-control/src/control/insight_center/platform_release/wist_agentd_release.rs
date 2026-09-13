// @jumo generated
#[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.InsightCenter.PlatformRelease")]
pub struct WistAgentdRelease {
    pub version: String,
    pub artifact_url: String,
    pub status: String,
    pub published_at: crate::DateTime,
}
