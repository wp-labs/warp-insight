// @moju generated
// @moju hash=0d635dd514b820fe

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct PreparedReport {
    pub origin: String,
    pub report: String,
}
