// @moju generated
// @moju hash=22828a9192739665

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct DiscRowContext {
    pub resource: String,
    pub target: String,
    pub candidate: String,
}
