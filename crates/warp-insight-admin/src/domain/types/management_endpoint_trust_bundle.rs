// @moju generated
// @moju hash=dc6752475e7ec954

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Control")]
pub struct ManagementEndpointTrustBundle {
    pub control_endpoint: String,
    pub expires_at: crate::domain::types::DateTime,
    #[moju(unique)]
    pub trust_bundle_id: String,
    pub ca_bundle: String,
    pub expected_san: String,
    pub issued_at: crate::domain::types::DateTime,
    pub server_name: String,
}
