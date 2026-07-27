// @moju generated
// @moju hash=2dd655e756f626e2

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "event", domain = "Reporting")]
pub struct DiscoveryIngestAck {
    pub report_id: String,
    pub status: String,
    pub ingested_resources: String,
    pub ingested_targets: String,
    pub received_at: String,
    pub ack_at: String,
}
