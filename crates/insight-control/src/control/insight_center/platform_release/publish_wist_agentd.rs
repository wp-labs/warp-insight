// @jumo generated
#[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "message", role = "command", domain = "Control", module = "Control.InsightCenter.PlatformRelease")]
pub struct PublishWistAgentd {
    pub version: String,
    pub artifact_url: String,
    pub requested_by: String,
    pub requested_at: crate::DateTime,
}
