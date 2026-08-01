// @moju generated
// @moju hash=b270264e12073c5d

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Reporting", module = "Reporting.Protocol")]
pub struct IngestHeadError {
    pub reason: String,
    pub detail: String,
}
