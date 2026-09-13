//! `warp-agentd` runtime loop and recovery helpers.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use wist_contracts::agent_config::AgentConfigContract;
use wist_contracts::gateway::{AgentHello, AgentWorkState, AgentWorkStateChange};
use wist_shared::time::now_rfc3339;

use crate::enrollment::enrollment_http_client;

use crate::error::RuntimeResult;

use crate::discovery::DiscoveryProbe;
use crate::discovery::container::ContainerDiscoveryProbe;
use crate::discovery::endpoint::EndpointDiscoveryProbe;
use crate::discovery::host::HostDiscoveryProbe;
use crate::discovery::network::NetworkDiscoveryProbe;
use crate::discovery::process::ProcessDiscoveryProbe;
use crate::discovery::runtime::{DiscoveryRefreshResult, DiscoveryRuntime};
use wist_contracts::exporter::ExporterSource;

use crate::exporter;
use crate::planner_bridge;
use crate::scheduler;
use crate::self_observability::{
    DiscoveryHealthSnapshot, DiscoveryProbeHealth, DiscoveryReadiness, HealthState,
    RuntimeHealthSnapshot, emit,
};
use crate::state_store::{agent_runtime, execution_queue, log_seq_state, planner_candidates};
use crate::telemetry::metrics::target_view;

#[path = "daemon_metrics.rs"]
mod metrics_support;
#[path = "daemon_recovery.rs"]
mod recovery_support;
#[path = "daemon_runtime_state.rs"]
mod runtime_state_support;
#[path = "daemon_telemetry.rs"]
mod telemetry_support;

/// How often the daemon reports its own status (memory / CPU / admin latency)
/// to the control plane.
const STATUS_REPORT_INTERVAL: Duration = Duration::from_secs(3);

/// A sampled CPU-time reading used to compute a percentage across the report interval.
struct CpuSample {
    ticks: u64,
    at: Instant,
}

/// Current resident set size in bytes, when the platform exposes it.
fn current_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string("/proc/self/statm").ok()?;
        // statm: size, resident, shared, text, lib, data, dt (all in pages).
        let resident_pages: u64 = content.split_whitespace().nth(1)?.parse().ok()?;
        Some(resident_pages * 4096)
    }
    #[cfg(target_os = "macos")]
    {
        // proc_pidinfo(PROC_PIDTASKINFO) reports the current resident size in bytes.
        let mut info: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
        let written = unsafe {
            libc::proc_pidinfo(
                std::process::id() as libc::c_int,
                libc::PROC_PIDTASKINFO,
                0,
                &mut info as *mut libc::proc_taskinfo as *mut libc::c_void,
                size,
            )
        };
        if written != size {
            return None;
        }
        Some(info.pti_resident_size)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// Total CPU time (user + system) of this process in clock ticks.
fn cpu_ticks() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string("/proc/self/stat").ok()?;
        // After the closing ')' of comm, fields are state(3), ppid(4), ...,
        // utime(14) at index 11, stime(15) at index 12.
        let fields: Vec<&str> = content.rsplit(')').next()?.split_whitespace().collect();
        let utime: u64 = fields.get(11)?.parse().ok()?;
        let stime: u64 = fields.get(12)?.parse().ok()?;
        Some(utime + stime)
    }
    #[cfg(target_os = "macos")]
    {
        // getrusage reports user + system CPU time; both are timeval (seconds
        // + microseconds). Total is expressed in microseconds so that
        // ticks_per_sec() = 1_000_000 makes cpu_percent_since() linear in
        // wall-clock seconds.
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } != 0 {
            return None;
        }
        let user_us = usage.ru_utime.tv_sec as i64 * 1_000_000 + usage.ru_utime.tv_usec as i64;
        let system_us = usage.ru_stime.tv_sec as i64 * 1_000_000 + usage.ru_stime.tv_usec as i64;
        Some((user_us + system_us) as u64)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
fn ticks_per_sec() -> u64 {
    unsafe { libc::sysconf(libc::_SC_CLK_TCK) as u64 }
}

#[cfg(target_os = "macos")]
fn ticks_per_sec() -> u64 {
    1_000_000
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn ticks_per_sec() -> u64 {
    100
}

fn cpu_percent_since(previous: &CpuSample, now: Instant, ticks_per_sec: u64) -> Option<f64> {
    let now_ticks = cpu_ticks()?;
    let wall = now.duration_since(previous.at).as_secs_f64();
    if wall <= 0.0 {
        return Some(0.0);
    }
    let tick_delta = now_ticks.saturating_sub(previous.ticks);
    Some(tick_delta as f64 / wall / ticks_per_sec as f64 * 100.0)
}

/// Best-effort status heartbeat to the admin control plane. Returns the measured
/// round-trip latency in milliseconds when the report succeeded.
async fn report_status_to_control_plane(
    config: &AgentConfigContract,
    cpu_percent: Option<f64>,
    last_latency_ms: Option<u64>,
    work_state_changes: Option<Vec<AgentWorkStateChange>>,
) -> Option<u64> {
    let Some(endpoint) = config.control_plane.endpoint.as_deref() else {
        return None;
    };
    let Some(bearer_token) = config.control_plane.bearer_token.as_deref() else {
        return None;
    };
    let Some(agent_id) = config.agent.agent_id.as_deref() else {
        return None;
    };
    let instance_id = config.agent.instance_name.as_deref().unwrap_or_default();
    let hello = AgentHello {
        agent_id: agent_id.to_string(),
        instance_id: instance_id.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        memory_bytes: current_rss_bytes(),
        cpu_percent,
        admin_latency_ms: last_latency_ms,
        work_state_changes,
    };
    let client = match enrollment_http_client(config) {
        Ok(client) => client,
        Err(err) => {
            eprintln!("warp-agentd status report: failed to build client: {err}");
            return None;
        }
    };
    let url = format!("{}/api/v1/agent/status", endpoint.trim_end_matches('/'));
    let started = Instant::now();
    match client
        .post(&url)
        .bearer_auth(bearer_token)
        .json(&hello)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            Some(started.elapsed().as_millis() as u64)
        }
        Ok(response) => {
            eprintln!(
                "warp-agentd status report failed: HTTP {} from {}",
                response.status(),
                endpoint
            );
            None
        }
        Err(err) => {
            eprintln!("warp-agentd status report failed: {err}");
            None
        }
    }
}

use metrics_support::{
    emit_metrics_failure, emit_metrics_failures, emit_metrics_tick,
    failure_signatures as metrics_failure_signatures,
    filter_new_failures as filter_new_metrics_failures, process_metrics_tick, write_metrics_uplink,
};
use recovery_support::recover_incomplete_executions_impl_async;
use runtime_state_support::{
    count_reporting_entries_async, count_running_entries_async, emit_telemetry_failure,
    emit_telemetry_failures, emit_work_state_notification, emit_work_state_notifications,
    failure_signatures, filter_new_failures, instance_id, paused_input_signatures,
    work_state_changes,
};
use telemetry_support::{
    TelemetryWorkState, WorkState, build_telemetry_sink, invalid_output_tick,
    process_telemetry_inputs,
};

fn to_agent_work_state_changes(changes: &[TelemetryWorkState]) -> Vec<AgentWorkStateChange> {
    changes
        .iter()
        .map(|change| AgentWorkStateChange {
            input_id: change.input_id.clone(),
            state: match change.state {
                WorkState::Paused => AgentWorkState::Paused,
                WorkState::Resumed => AgentWorkState::Resumed,
            },
            reason: change.reason.clone(),
            at: change.at.clone(),
        })
        .collect()
}

#[derive(::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
pub struct DaemonLoop<'a> {
    pub config: &'a AgentConfigContract,
    pub exec_bin: &'a Path,
}

pub async fn run_forever_async(loop_ctx: DaemonLoop<'_>) -> RuntimeResult<()> {
    let sleep_interval = Duration::from_millis(250);
    let mut previous_telemetry_failures = BTreeSet::new();
    let mut previous_metrics_failures = BTreeSet::new();
    let mut previous_telemetry_paused = BTreeSet::new();
    let mut last_report_at = Instant::now();
    let mut last_cpu_sample = cpu_ticks().map(|ticks| CpuSample {
        ticks,
        at: Instant::now(),
    });
    let mut last_latency_ms: Option<u64> = None;
    let mut pending_work_state_changes: Vec<TelemetryWorkState> = Vec::new();
    loop {
        let (snapshot, changes) = run_once_with_failure_cache(
            &loop_ctx,
            Some(&mut previous_telemetry_failures),
            Some(&mut previous_metrics_failures),
            Some(&mut previous_telemetry_paused),
        )
        .await?;
        pending_work_state_changes.extend(changes);
        emit(&snapshot);
        if last_report_at.elapsed() >= STATUS_REPORT_INTERVAL {
            last_report_at = Instant::now();
            let now = Instant::now();
            let cpu_percent = last_cpu_sample
                .as_ref()
                .and_then(|sample| cpu_percent_since(sample, now, ticks_per_sec()));
            if let Some(ticks) = cpu_ticks() {
                last_cpu_sample = Some(CpuSample { ticks, at: now });
            }
            let changes = std::mem::take(&mut pending_work_state_changes);
            let changes = if changes.is_empty() {
                None
            } else {
                Some(to_agent_work_state_changes(&changes))
            };
            if let Some(latency) = report_status_to_control_plane(
                loop_ctx.config,
                cpu_percent,
                last_latency_ms,
                changes,
            )
            .await
            {
                last_latency_ms = Some(latency);
            }
        }
        tokio::time::sleep(sleep_interval).await;
    }
}

pub async fn run_once_async(loop_ctx: &DaemonLoop<'_>) -> RuntimeResult<RuntimeHealthSnapshot> {
    run_once_with_failure_cache(loop_ctx, None, None, None)
        .await
        .map(|(snapshot, _)| snapshot)
}

async fn run_once_with_failure_cache(
    loop_ctx: &DaemonLoop<'_>,
    previous_telemetry_failures: Option<&mut BTreeSet<String>>,
    previous_metrics_failures: Option<&mut BTreeSet<String>>,
    previous_telemetry_paused: Option<&mut BTreeSet<String>>,
) -> RuntimeResult<(RuntimeHealthSnapshot, Vec<TelemetryWorkState>)> {
    let run_dir = Path::new(&loop_ctx.config.paths.run_dir);
    let state_dir = Path::new(&loop_ctx.config.paths.state_dir);
    let instance_id = instance_id(loop_ctx.config);
    let agent_id = loop_ctx
        .config
        .agent
        .agent_id
        .as_deref()
        .unwrap_or("unknown");
    let discovery = refresh_discovery_snapshot(loop_ctx.config, state_dir).await;
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
    let global_seq_path = log_seq_state::path_for(state_dir);
    let mut next_seq = log_seq_state::load_or_default_async(&global_seq_path)
        .await
        .unwrap_or(0);
    let telemetry_tick = match build_telemetry_sink(loop_ctx.config) {
        Ok(mut sink) => {
            // 指标优先：先上送指标帧（与日志共用同一 sink/连接 + 同一个全局 seq），再处理日志。
            if let Some(snapshot) = metrics_tick.snapshot.as_ref() {
                if let Err(err) = write_metrics_uplink(
                    &mut sink,
                    agent_id,
                    snapshot,
                    &mut next_seq,
                    &global_seq_path,
                )
                .await
                {
                    eprintln!("warp-agentd metrics uplink failed: {err}");
                }
            }
            process_telemetry_inputs(loop_ctx.config, &mut sink, &mut next_seq).await
        }
        Err(err) => invalid_output_tick(loop_ctx.config, err.to_string()),
    };
    if let Some(previous) = previous_telemetry_failures {
        for failure in filter_new_failures(&telemetry_tick.failures, previous) {
            emit_telemetry_failure(failure);
        }
        *previous = failure_signatures(&telemetry_tick.failures);
    } else {
        emit_telemetry_failures(&telemetry_tick.failures);
    }
    let current_paused = paused_input_signatures(&telemetry_tick.notifications);
    let tick_changes = if let Some(previous) = previous_telemetry_paused {
        let changes = work_state_changes(previous, &telemetry_tick.notifications, &current_paused);
        for change in &changes {
            emit_work_state_notification(change);
        }
        *previous = current_paused.clone();
        changes
    } else {
        emit_work_state_notifications(&telemetry_tick.notifications);
        Vec::new()
    };
    let telemetry_active = telemetry_tick.is_active();
    let metrics_active = metrics_tick.is_active();

    recover_incomplete_executions_impl_async(state_dir, &instance_id).await?;

    // Step 0: export unified-envelope output alongside existing cache files
    let export_source = ExporterSource::new(agent_id, &instance_id);
    exporter::export_all_async(state_dir, &export_source).await;

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

    let queue =
        execution_queue::load_or_default_async(&execution_queue::path_for(state_dir)).await?;
    let running_count = count_running_entries_async(state_dir).await?;
    let reporting_count = count_reporting_entries_async(state_dir).await?;
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
        paused_inputs: current_paused.into_iter().collect(),
        discovery: discovery.snapshot,
        metrics,
        updated_at: now_rfc3339(),
    };

    let runtime_path = agent_runtime::path_for(state_dir);
    let mut runtime_state = agent_runtime::load_or_default_async(&runtime_path).await?;
    runtime_state.updated_at = health.updated_at.clone();
    agent_runtime::store_async(&runtime_path, &runtime_state).await?;

    Ok((health, tick_changes))
}

#[derive(::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
struct DiscoveryHealth {
    snapshot: DiscoveryHealthSnapshot,
}

async fn refresh_discovery_snapshot(
    config: &AgentConfigContract,
    state_dir: &Path,
) -> DiscoveryHealth {
    let mut runtime = DiscoveryRuntime::new(discovery_probes(config));
    let (cached, cache_load_failure) = runtime.load_from_state_dir_async(state_dir).await;
    let (cached_meta, meta_load_failure) = runtime.load_meta_from_state_dir_async(state_dir).await;
    let mut result = runtime.refresh_and_store_async(state_dir).await;
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
    let planner_store_result: io::Result<()> = async {
        planner_candidates::store_async(
            &planner_candidates::host_metrics_path_for(state_dir),
            &host_candidates,
        )
        .await?;
        planner_candidates::store_async(
            &planner_candidates::process_metrics_path_for(state_dir),
            &process_candidates,
        )
        .await?;
        planner_candidates::store_async(
            &planner_candidates::container_metrics_path_for(state_dir),
            &container_candidates,
        )
        .await
    }
    .await;
    if let Err(err) = planner_store_result {
        result.last_error = Some(format!("planner candidate store failed: {err}"));
        result.store_failure = Some(crate::discovery::runtime::DiscoveryStoreFailure {
            phase: "planner_store",
            detail: format!("planner candidate store failed: {err}"),
        });
    } else {
        let target_view_result: io::Result<()> = async {
            let view = target_view::build_metrics_target_view_async(
                state_dir,
                &result.persisted_snapshot.generated_at,
            )
            .await?;
            target_view::store_async(&target_view::path_for(state_dir), &view).await
        }
        .await;
        if let Err(err) = target_view_result {
            result.last_error = Some(format!("metrics target view store failed: {err}"));
            result.store_failure = Some(crate::discovery::runtime::DiscoveryStoreFailure {
                phase: "metrics_target_view_store",
                detail: format!("metrics target view store failed: {err}"),
            });
        }
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

    DiscoveryHealth {
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
    }
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
        let source = error
            .context_metadata()
            .get_str("source")
            .unwrap_or("unknown")
            .to_string();
        let probe = error
            .context_metadata()
            .get_str("probe")
            .unwrap_or("unknown")
            .to_string();
        let phase = "refresh".to_string();
        let detail = error.detail().clone().unwrap_or_else(|| error.to_string());
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

pub fn run_once(loop_ctx: &DaemonLoop<'_>) -> RuntimeResult<RuntimeHealthSnapshot> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run_once_async(loop_ctx))
}

pub fn recover_incomplete_executions(state_dir: &Path, instance_id: &str) -> RuntimeResult<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(recover_incomplete_executions_impl_async(
            state_dir,
            instance_id,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use wist_contracts::agent_config::{
        AgentSection, ControlPlaneSection, ExecutionSection, PathsSection,
    };

    fn test_config() -> AgentConfigContract {
        AgentConfigContract::new(
            AgentSection {
                agent_id: Some("agent-x".to_string()),
                environment_id: None,
                instance_name: Some("instance-x".to_string()),
            },
            ControlPlaneSection {
                enabled: true,
                endpoint: Some("http://127.0.0.1:1".to_string()),
                enrollment_token: None,
                credential_request: None,
                credential_id: None,
                bearer_token: Some("wic_test_token".to_string()),
                credential_expires_at: None,
                tls_mode: None,
                trust_bundle: None,
                auth_mode: None,
            },
            PathsSection {
                root_dir: ".".to_string(),
                run_dir: "run".to_string(),
                state_dir: "state".to_string(),
                log_dir: "log".to_string(),
            },
            ExecutionSection {
                max_running_actions: 1,
                cancel_grace_ms: 5_000,
                default_stdout_limit_bytes: 1,
                default_stderr_limit_bytes: 1,
            },
        )
    }

    async fn read_http_request(socket: &mut tokio::net::TcpStream) -> String {
        let mut request_bytes = Vec::new();
        loop {
            let mut chunk = [0u8; 1024];
            let read = socket.read(&mut chunk).await.expect("read");
            if read == 0 {
                break;
            }
            request_bytes.extend_from_slice(&chunk[..read]);
            let Some(header_end) = request_bytes.windows(4).position(|w| w == b"\r\n\r\n") else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request_bytes[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    if key.trim().eq_ignore_ascii_case("content-length") {
                        value.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if request_bytes.len() >= header_end + 4 + content_length {
                break;
            }
        }
        String::from_utf8_lossy(&request_bytes).into_owned()
    }

    #[tokio::test]
    async fn report_status_posts_agent_hello_with_metrics() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let endpoint = format!("http://{}", listener.local_addr().expect("addr"));
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let request = read_http_request(&mut socket).await;
            assert!(request.contains("/api/v1/agent/status"));
            assert!(
                request
                    .to_lowercase()
                    .contains("authorization: bearer wic_test_token")
            );
            assert!(request.contains("\"agent_id\":\"agent-x\""));
            assert!(request.contains("\"memory_bytes\":"));
            assert!(request.contains("\"cpu_percent\":"));
            assert!(request.contains("\"admin_latency_ms\":"));
            assert!(request.contains("\"work_state_changes\":null"));
            let response =
                "HTTP/1.1 202 Accepted\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
            socket.write_all(response.as_bytes()).await.expect("write");
        });

        let mut config = test_config();
        config.control_plane.endpoint = Some(endpoint);
        let latency = report_status_to_control_plane(&config, Some(12.5), Some(3), None).await;
        server.await.expect("server task");
        assert!(latency.is_some());
    }

    #[tokio::test]
    async fn report_status_posts_work_state_changes() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let endpoint = format!("http://{}", listener.local_addr().expect("addr"));
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let request = read_http_request(&mut socket).await;
            assert!(request.contains("\"work_state_changes\":["));
            assert!(request.contains("\"input_id\":\"app\""));
            assert!(request.contains("\"state\":\"paused\""));
            let response =
                "HTTP/1.1 202 Accepted\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
            socket.write_all(response.as_bytes()).await.expect("write");
        });

        let mut config = test_config();
        config.control_plane.endpoint = Some(endpoint);
        let changes = Some(vec![wist_contracts::gateway::AgentWorkStateChange {
            input_id: "app".to_string(),
            state: wist_contracts::gateway::AgentWorkState::Paused,
            reason: "spool over limit".to_string(),
            at: "now".to_string(),
        }]);
        let latency = report_status_to_control_plane(&config, Some(12.5), Some(3), changes).await;
        server.await.expect("server task");
        assert!(latency.is_some());
    }

    #[tokio::test]
    async fn report_status_skips_when_not_enrolled() {
        let mut config = test_config();
        config.control_plane.bearer_token = None;
        let latency = report_status_to_control_plane(&config, None, None, None).await;
        assert!(latency.is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_resource_measurements_return_values() {
        let rss = current_rss_bytes().expect("current_rss_bytes on macos");
        assert!(rss > 0, "resident memory should be positive, got {rss}");
        let first = cpu_ticks().expect("cpu_ticks on macos");
        let second = cpu_ticks().expect("cpu_ticks on macos");
        assert!(second >= first, "cpu time should not decrease");
        assert_eq!(ticks_per_sec(), 1_000_000);
    }
}
