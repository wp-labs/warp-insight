//! Self-observability placeholders.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryReadiness {
    NotReady,
    ReadyWithStaleSnapshot,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Health")]
pub struct DiscoveryProbeHealth {
    pub source: String,
    pub probe: String,
    pub phase: String,
    pub status: String,
    pub resource_count: usize,
    pub target_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Health")]
pub struct DiscoveryHealthSnapshot {
    pub readiness: DiscoveryReadiness,
    pub cached_snapshot_loaded: bool,
    pub used_cached_snapshot: bool,
    pub resource_count: usize,
    pub target_count: usize,
    pub failure_count: usize,
    pub last_success_at: Option<String>,
    pub updated_at: String,
    pub probes: Vec<DiscoveryProbeHealth>,
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Health")]
pub struct MetricsHealthSnapshot {
    pub target_view_loaded: bool,
    pub used_cached_snapshot: bool,
    pub total_targets: usize,
    pub host_targets: usize,
    pub process_targets: usize,
    pub container_targets: usize,
    pub attempted_targets: usize,
    pub succeeded_targets: usize,
    pub failed_targets: usize,
    pub failure_count: usize,
    pub last_error: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthState {
    Idle,
    Active,
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Health")]
pub struct RuntimeHealthSnapshot {
    pub state: HealthState,
    pub queue_depth: usize,
    pub running_count: usize,
    pub reporting_count: usize,
    pub discovery: DiscoveryHealthSnapshot,
    pub metrics: MetricsHealthSnapshot,
    pub updated_at: String,
}

pub fn register() {
    eprintln!("self-observability registered");
}

pub fn emit(snapshot: &RuntimeHealthSnapshot) {
    eprintln!(
        "health state={:?} queue={} running={} reporting={} discovery_readiness={:?} discovery_cached_loaded={} discovery_used_cached={} discovery_resources={} discovery_targets={} discovery_failures={} discovery_last_success_at={} updated_at={}",
        snapshot.state,
        snapshot.queue_depth,
        snapshot.running_count,
        snapshot.reporting_count,
        snapshot.discovery.readiness,
        snapshot.discovery.cached_snapshot_loaded,
        snapshot.discovery.used_cached_snapshot,
        snapshot.discovery.resource_count,
        snapshot.discovery.target_count,
        snapshot.discovery.failure_count,
        snapshot.discovery.last_success_at.as_deref().unwrap_or("-"),
        snapshot.updated_at
    );
    eprintln!(
        "metrics_runtime target_view_loaded={} used_cached_snapshot={} total_targets={} host_targets={} process_targets={} container_targets={} attempted_targets={} succeeded_targets={} failed_targets={} failures={} last_error={} updated_at={}",
        snapshot.metrics.target_view_loaded,
        snapshot.metrics.used_cached_snapshot,
        snapshot.metrics.total_targets,
        snapshot.metrics.host_targets,
        snapshot.metrics.process_targets,
        snapshot.metrics.container_targets,
        snapshot.metrics.attempted_targets,
        snapshot.metrics.succeeded_targets,
        snapshot.metrics.failed_targets,
        snapshot.metrics.failure_count,
        snapshot.metrics.last_error.as_deref().unwrap_or("-"),
        snapshot.metrics.updated_at.as_deref().unwrap_or("-"),
    );

    for probe in &snapshot.discovery.probes {
        eprintln!(
            "discovery_probe source={} probe={} phase={} status={} resources={} targets={} error={}",
            probe.source,
            probe.probe,
            probe.phase,
            probe.status,
            probe.resource_count,
            probe.target_count,
            probe.error.as_deref().unwrap_or("-"),
        );
    }
}
