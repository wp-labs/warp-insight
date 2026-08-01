// @moju generated
// @moju hash=c7fb3e88f7c747e0

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct ActionPlanStep {
    pub op: String,
    #[moju(unique)]
    pub id: String,
    pub kind: String,
}
