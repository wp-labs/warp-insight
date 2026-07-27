// @moju generated
// @moju hash=dbdabdd61754ddd4

#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting")]
pub struct DiscoveryHealthSnapshot {
    pub readiness: String,
    pub resource_count: String,
    pub updated_at: String,
    pub target_count: String,
    pub failure_count: String,
    pub cached_snapshot_loaded: String,
    pub used_cached_snapshot: String,
    pub probes: String,
    pub last_success_at: String,
}
