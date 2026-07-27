// @moju generated
// @moju hash=426210233649fc5b

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct ResultAttestation {
    pub issued_by: String,
    pub attested_at: crate::domain::types::DateTime,
    pub result_digest: String,
    pub signature: String,
}
