//! `warp-insightd` runtime loop and recovery helpers.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::time::Duration;

use warp_insight_contracts::agent_config::AgentConfigContract;
use warp_insight_shared::time::now_rfc3339;

use crate::discovery::DiscoveryProbe;
use crate::discovery::container::ContainerDiscoveryProbe;
use crate::discovery::endpoint::EndpointDiscoveryProbe;
use crate::discovery::host::HostDiscoveryProbe;
use crate::discovery::network::NetworkDiscoveryProbe;
use crate::discovery::process::ProcessDiscoveryProbe;
use crate::discovery::runtime::{DiscoveryRefreshResult, DiscoveryRuntime};
use warp_insight_contracts::exporter::ExporterSource;

use crate::exporter;
use crate::planner_bridge;
use crate::scheduler;
use crate::self_observability::{
    DiscoveryHealthSnapshot, DiscoveryProbeHealth, DiscoveryReadiness, HealthState,
    RuntimeHealthSnapshot, emit,
};
use crate::state_store::{agent_runtime, execution_queue, planner_candidates};
use crate::telemetry::metrics::target_view;

#[path = "daemon_metrics.rs"]
mod metrics_support;
#[path = "daemon_recovery.rs"]
mod recovery_support;
#[path = "daemon_runtime_state.rs"]
mod runtime_state_support;
#[path = "daemon_telemetry.rs"]
mod telemetry_support;

use metrics_support::{
    emit_metrics_failure, emit_metrics_failures, emit_metrics_tick,
    failure_signatures as metrics_failure_signatures,
    filter_new_failures as filter_new_metrics_failures, process_metrics_tick,
};
use recovery_support::recover_incomplete_executions_impl;
use runtime_state_support::{
    count_reporting_entries, count_running_entries, emit_telemetry_failure,
    emit_telemetry_failures, failure_signatures, filter_new_failures, instance_id,
};
use telemetry_support::process_telemetry_inputs;

#[derive(::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
pub struct DaemonLoop<'a> {
    pub config: &'a AgentConfigContract,
    pub exec_bin: &'a Path,
}

pub async fn run_forever_async(loop_ctx: DaemonLoop<'_>) -> io::Result<()> {
    let sleep_interval = Duration::from_millis(250);
    let mut previous_telemetry_failures = BTreeSet::new();
    let mut previous_metrics_failures = BTreeSet::new();
    loop {
        let snapshot = run_once_with_failure_cache(
            &loop_ctx,
            Some(&mut previous_telemetry_failures),
            Some(&mut previous_metrics_failures),
        )
        .await?;
        emit(&snapshot);
        tokio::time::sleep(sleep_interval).await;
    }
}

pub async fn run_once_async(loop_ctx: &DaemonLoop<'_>) -> io::Result<RuntimeHealthSnapshot> {
    run_once_with_failure_cache(loop_ctx, None, None).await
}

async fn run_once_with_failure_cache(
    loop_ctx: &DaemonLoop<'_>,
    previous_telemetry_failures: Option<&mut BTreeSet<String>>,
    previous_metrics_failures: Option<&mut BTreeSet<String>>,
) -> io::Result<RuntimeHealthSnapshot> {
    let run_dir = Path::new(&loop_ctx.config.paths.run_dir);
    let state_dir = Path::new(&loop_ctx.config.paths.state_dir);
    let instance_id = instance_id(loop_ctx.config);
    let discovery = refresh_discovery_snapshot(loop_ctx.config, state_dir)?;
    let metrics_tick = process_metrics_tick(state_dir);
    emit_metrics_tick(&metrics_tick);
    if let Some(previous) = previous_metrics_failures {
        for failure in filter_new_metrics_failures(&metrics_tick.failures, previous) {
            emit_metrics_failure(failure);
        }
        *previous = metrics_failure_signatures(&metrics_tick.failures);
    } else {
        emit_metrics_failures(&metrics_tick.failures);
    }
    let telemetry_tick = process_telemetry_inputs(loop_ctx.config).await;
    if let Some(previous) = previous_telemetry_failures {
        for failure in filter_new_failures(&telemetry_tick.failures, previous) {
            emit_telemetry_failure(failure);
        }
        *previous = failure_signatures(&telemetry_tick.failures);
    } else {
        emit_telemetry_failures(&telemetry_tick.failures);
    }
    let telemetry_active = telemetry_tick.is_active();
    let metrics_active = metrics_tick.is_active();

    recover_incomplete_executions(state_dir, &instance_id)?;

    // Step 0: export unified-envelope output alongside existing cache files
    let agent_id = loop_ctx
        .config
        .agent
        .agent_id
        .as_deref()
        .unwrap_or("unknown");
    let export_source = ExporterSource::new(agent_id, &instance_id);
    exporter::export_all(state_dir, &export_source);

    let drained = scheduler::drain_next_async(&scheduler::DrainRequest {
        run_dir: run_dir.to_path_buf(),
        state_dir: state_dir.to_path_buf(),
        exec_bin: loop_ctx.exec_bin.to_path_buf(),
        instance_id,
        cancel_grace_ms: loop_ctx.config.execution.cancel_grace_ms,
        stdout_limit_bytes: loop_ctx.config.execution.default_stdout_limit_bytes,
        stderr_limit_bytes: loop_ctx.config.execution.default_stderr_limit_bytes,
    })
    .await?;

    let queue = execution_queue::load_or_default(&execution_queue::path_for(state_dir))?;
    let running_count = count_running_entries(state_dir)?;
    let reporting_count = count_reporting_entries(state_dir)?;
    let metrics = metrics_tick.health_snapshot();
    let health = RuntimeHealthSnapshot {
        state: if telemetry_active
            || metrics_active
            || drained
            || running_count > 0
            || reporting_count > 0
            || !queue.items.is_empty()
        {
            HealthState::Active
        } else {
            HealthState::Idle
        },
        queue_depth: queue.items.len(),
        running_count,
        reporting_count,
        discovery: discovery.snapshot,
        metrics,
        updated_at: now_rfc3339(),
    };

    let runtime_path = agent_runtime::path_for(state_dir);
    let mut runtime_state = agent_runtime::load_or_default(&runtime_path)?;
    runtime_state.updated_at = health.updated_at.clone();
    agent_runtime::store(&runtime_path, &runtime_state)?;

    Ok(health)
}

#[derive(::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
struct DiscoveryHealth {
    snapshot: DiscoveryHealthSnapshot,
}

fn refresh_discovery_snapshot(
    config: &AgentConfigContract,
    state_dir: &Path,
) -> io::Result<DiscoveryHealth> {
    let mut runtime = DiscoveryRuntime::new(discovery_probes(config));
    let (cached, cache_load_failure) = runtime.load_from_state_dir(state_dir)?;
    let (cached_meta, meta_load_failure) = runtime.load_meta_from_state_dir(state_dir)?;
    let mut result = runtime.refresh_and_store(state_dir)?;
    let candidates = planner_bridge::build_collection_candidates(&result.persisted_snapshot);
    let host_candidates: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.collection_kind == "host_metrics")
        .cloned()
        .collect();
    let process_candidates: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.collection_kind == "process_metrics")
        .cloned()
        .collect();
    let container_candidates: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.collection_kind == "container_metrics")
        .cloned()
        .collect();
    let planner_store_result = planner_candidates::store(
        &planner_candidates::host_metrics_path_for(state_dir),
        &host_candidates,
    )
    .and_then(|_| {
        planner_candidates::store(
            &planner_candidates::process_metrics_path_for(state_dir),
            &process_candidates,
        )
    })
    .and_then(|_| {
        planner_candidates::store(
            &planner_candidates::container_metrics_path_for(state_dir),
            &container_candidates,
        )
    });
    if let Err(err) = planner_store_result {
        result.last_error = Some(format!("planner candidate store failed: {err}"));
        result.store_failure = Some(crate::discovery::runtime::DiscoveryStoreFailure {
            phase: "planner_store",
            detail: format!("planner candidate store failed: {err}"),
        });
    } else if let Err(err) =
        target_view::build_metrics_target_view(state_dir, &result.persisted_snapshot.generated_at)
            .and_then(|view| target_view::store(&target_view::path_for(state_dir), &view))
    {
        result.last_error = Some(format!("metrics target view store failed: {err}"));
        result.store_failure = Some(crate::discovery::runtime::DiscoveryStoreFailure {
            phase: "metrics_target_view_store",
            detail: format!("metrics target view store failed: {err}"),
        });
    }
    let probes = build_probe_health(
        &result,
        cache_load_failure.as_ref(),
        meta_load_failure.as_ref(),
    );
    emit_discovery_refresh(&result, &probes);

    let readiness = if result.used_cached_snapshot {
        DiscoveryReadiness::ReadyWithStaleSnapshot
    } else if result.had_successful_refresh {
        DiscoveryReadiness::Ready
    } else if cached.is_some() {
        DiscoveryReadiness::ReadyWithStaleSnapshot
    } else {
        DiscoveryReadiness::NotReady
    };

    Ok(DiscoveryHealth {
        snapshot: DiscoveryHealthSnapshot {
            readiness,
            cached_snapshot_loaded: cached.is_some(),
            used_cached_snapshot: result.used_cached_snapshot,
            resource_count: result.persisted_snapshot.resources.len(),
            target_count: result.persisted_snapshot.targets.len(),
            failure_count: probes
                .iter()
                .filter(|probe| probe.status == "failed")
                .count(),
            last_success_at: result
                .last_success_at
                .clone()
                .or_else(|| cached_meta.and_then(|meta| meta.last_success_at)),
            updated_at: result.refreshed_snapshot.generated_at.clone(),
            probes,
        },
    })
}

fn discovery_probes(config: &AgentConfigContract) -> Vec<Box<dyn DiscoveryProbe + Send + Sync>> {
    let mut probes: Vec<Box<dyn DiscoveryProbe + Send + Sync>> = Vec::new();
    if config.discovery.host_enabled {
        probes.push(Box::new(HostDiscoveryProbe));
    }
    if config.discovery.network_enabled {
        probes.push(Box::new(NetworkDiscoveryProbe));
    }
    if config.discovery.endpoint_enabled {
        probes.push(Box::new(EndpointDiscoveryProbe));
    }
    if config.discovery.process_enabled {
        probes.push(Box::new(ProcessDiscoveryProbe));
    }
    if config.discovery.container_enabled {
        probes.push(Box::new(ContainerDiscoveryProbe));
    }
    probes
}

fn build_probe_health(
    result: &DiscoveryRefreshResult,
    cache_load_failure: Option<&crate::discovery::cache::DiscoveryCacheLoadFailure>,
    meta_load_failure: Option<&crate::discovery::cache::DiscoveryCacheLoadFailure>,
) -> Vec<DiscoveryProbeHealth> {
    let mut probes = Vec::new();
    let mut seen_failures = std::collections::BTreeSet::new();

    for successful in &result.successful_probes {
        probes.push(DiscoveryProbeHealth {
            source: successful.source.as_str().to_string(),
            probe: successful.probe.clone(),
            phase: "refresh".to_string(),
            status: "ok".to_string(),
            resource_count: successful.resource_count,
            target_count: successful.target_count,
            error: None,
        });
    }

    for error in &result.errors {
        let source = error.source.as_str().to_string();
        let probe = error.probe.clone();
        let phase = "refresh".to_string();
        let detail = error.detail.clone();
        if seen_failures.insert((source.clone(), probe.clone(), phase.clone(), detail.clone())) {
            probes.push(DiscoveryProbeHealth {
                source,
                probe,
                phase,
                status: "failed".to_string(),
                resource_count: 0,
                target_count: 0,
                error: Some(detail),
            });
        }
    }

    if let Some(store_failure) = &result.store_failure {
        let source = "cache".to_string();
        let probe = "discovery".to_string();
        let phase = store_failure.phase.to_string();
        let detail = store_failure.detail.clone();
        if seen_failures.insert((source.clone(), probe.clone(), phase.clone(), detail.clone())) {
            probes.push(DiscoveryProbeHealth {
                source,
                probe,
                phase,
                status: "failed".to_string(),
                resource_count: 0,
                target_count: 0,
                error: Some(detail),
            });
        }
    }

    for load_failure in [cache_load_failure, meta_load_failure]
        .into_iter()
        .flatten()
    {
        let source = "cache".to_string();
        let probe = "discovery".to_string();
        let phase = load_failure.phase.to_string();
        let detail = load_failure.detail.clone();
        if seen_failures.insert((source.clone(), probe.clone(), phase.clone(), detail.clone())) {
            probes.push(DiscoveryProbeHealth {
                source,
                probe,
                phase,
                status: "failed".to_string(),
                resource_count: 0,
                target_count: 0,
                error: Some(detail),
            });
        }
    }

    probes
}

fn emit_discovery_refresh(result: &DiscoveryRefreshResult, probes: &[DiscoveryProbeHealth]) {
    eprintln!(
        "event=DiscoveryRefreshed revision={} persisted_revision={} resources={} targets={} failures={} used_cached_snapshot={} last_success_at={}",
        result.refreshed_snapshot.revision,
        result.persisted_snapshot.revision,
        result.persisted_snapshot.resources.len(),
        result.persisted_snapshot.targets.len(),
        result.errors.len(),
        result.used_cached_snapshot,
        result.last_success_at.as_deref().unwrap_or("-"),
    );

    for probe in probes {
        if probe.status == "failed" {
            eprintln!(
                "event=DiscoveryRefreshFailed source={} probe={} phase={} error={}",
                probe.source,
                probe.probe,
                probe.phase,
                probe.error.as_deref().unwrap_or("-"),
            );
        }
    }
}

pub fn run_once(loop_ctx: &DaemonLoop<'_>) -> io::Result<RuntimeHealthSnapshot> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run_once_async(loop_ctx))
}

pub fn recover_incomplete_executions(state_dir: &Path, instance_id: &str) -> io::Result<()> {
    recover_incomplete_executions_impl(state_dir, instance_id)
}
