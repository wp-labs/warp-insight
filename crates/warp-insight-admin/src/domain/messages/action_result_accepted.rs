// @moju generated
// @moju hash=f74bf8b38ab755e5

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "message", role = "response", domain = "Reporting", module = "Reporting.Protocol")]
pub struct ActionResultAccepted {
    pub receipt: crate::domain::types::ActionResultReceipt,
}
