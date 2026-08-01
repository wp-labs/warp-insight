// @moju generated
// @moju hash=ce5a7af2683d15db

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct ActionOutputItem {
    pub value: String,
    pub redacted: String,
    pub name: String,
}
