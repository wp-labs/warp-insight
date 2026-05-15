use std::fs;
use std::io::Read;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use warp_insight_contracts::agent_config::{DiscoverySection, LogFileInputSection};
use warp_insight_contracts::discovery::{
    CandidateCollectionTarget, DiscoveredResource, DiscoveredTarget, DiscoveryCacheMeta,
};
use warp_insight_contracts::telemetry_record::TelemetryRecordContract;
use warp_insight_shared::fs::read_json;
use warp_insightd::bootstrap;
use warp_insightd::daemon;
use warp_insightd::self_observability::DiscoveryReadiness;

use super::common::{
    TestLogCheckpointState, standalone_config_with_file_input, standalone_config_with_file_inputs,
    standalone_config_with_tcp_file_input, temp_dir, test_exec_bin,
};

fn bind_tcp_listener(addr: &str) -> Option<TcpListener> {
    match TcpListener::bind(addr) {
        Ok(listener) => Some(listener),
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => None,
        Err(err) => panic!("bind tcp listener: {err}"),
    }
}

#[derive(Debug, Deserialize)]
struct TestMetricsTargetView {
    targets: Vec<TestMetricsTargetViewEntry>,
}

#[derive(Debug, Deserialize)]
struct TestMetricsTargetViewEntry {
    collection_kind: String,
}

#[derive(Debug, Deserialize)]
struct TestMetricsRuntimeSnapshot {
    total_targets: usize,
    host_targets: usize,
    process_targets: usize,
    container_targets: usize,
    outcomes: Vec<TestMetricsCollectionOutcome>,
}

#[derive(Debug, Deserialize)]
struct TestMetricsCollectionOutcome {
    collection_kind: String,
    status: String,
    attempted_targets: usize,
    succeeded_targets: usize,
    failed_targets: usize,
    last_error: Option<String>,
    runtime_facts: Vec<warp_insight_contracts::discovery::StringKeyValue>,
    sample_targets: Vec<TestMetricsCollectionTargetSample>,
}

#[derive(Debug, Deserialize)]
struct TestMetricsCollectionTargetSample {
    candidate_id: String,
    target_ref: String,
}

#[cfg(unix)]
#[test]
fn daemon_run_once_processes_configured_file_input() {
    let root = temp_dir("daemon-file-input");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    let input_path = root.join("app.log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");
    fs::write(&input_path, "first\nsecond\n").expect("write input log");

    let config = standalone_config_with_file_input(&root, &input_path);
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("daemon run once");

    let output_path = root.join("log").join("warp-parse-records.ndjson");
    let output = fs::read_to_string(&output_path).expect("read output");
    let records: Vec<warp_insight_contracts::telemetry_record::TelemetryRecordContract> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse telemetry record"))
        .collect();
    let checkpoint_path = warp_insightd::state_store::log_checkpoints::path_for(&state_dir, "app");
    let checkpoint: TestLogCheckpointState = read_json(&checkpoint_path).expect("read checkpoint");
    let discovery_root = state_dir.join("discovery");
    let discovery_resources: Vec<DiscoveredResource> =
        read_json(&discovery_root.join("resources.json")).expect("read discovery resources");
    let discovery_targets: Vec<DiscoveredTarget> =
        read_json(&discovery_root.join("targets.json")).expect("read discovery targets");
    let discovery_meta: DiscoveryCacheMeta =
        read_json(&discovery_root.join("meta.json")).expect("read discovery meta");
    let host_planner_candidates: Vec<CandidateCollectionTarget> = read_json(
        &state_dir
            .join("planner")
            .join("host_metrics_candidates.json"),
    )
    .expect("read host planner candidates");
    let process_planner_candidates: Vec<CandidateCollectionTarget> = read_json(
        &state_dir
            .join("planner")
            .join("process_metrics_candidates.json"),
    )
    .expect("read process planner candidates");
    let container_planner_candidates: Vec<CandidateCollectionTarget> = read_json(
        &state_dir
            .join("planner")
            .join("container_metrics_candidates.json"),
    )
    .expect("read container planner candidates");
    let metrics_target_view: TestMetricsTargetView =
        read_json(&state_dir.join("telemetry").join("metrics_target_view.json"))
            .expect("read metrics target view");
    let metrics_runtime_snapshot: TestMetricsRuntimeSnapshot = read_json(
        &state_dir
            .join("telemetry")
            .join("metrics_runtime_snapshot.json"),
    )
    .expect("read metrics runtime snapshot");
    let _metrics_samples_exporter = state_dir.join("export").join("metrics.jsonl");

    assert_eq!(
        snapshot.state,
        warp_insightd::self_observability::HealthState::Active
    );
    assert_eq!(snapshot.discovery.readiness, DiscoveryReadiness::Ready);
    assert!(!snapshot.discovery.used_cached_snapshot);
    assert!(snapshot.discovery.failure_count <= 1);
    assert!(snapshot.metrics.target_view_loaded);
    assert!(!snapshot.metrics.used_cached_snapshot);
    assert_eq!(
        snapshot.metrics.attempted_targets,
        metrics_runtime_snapshot.total_targets
    );
    assert_eq!(
        snapshot.metrics.succeeded_targets + snapshot.metrics.failed_targets,
        metrics_runtime_snapshot.total_targets
    );
    assert_eq!(snapshot.metrics.failure_count, 0);
    assert!(!snapshot.discovery.probes.is_empty());
    assert!(snapshot.discovery.probes.iter().any(|probe| {
        probe.source == "local_runtime"
            && probe.probe == "host"
            && probe.phase == "refresh"
            && probe.status == "ok"
    }));
    assert!(
        snapshot
            .discovery
            .probes
            .iter()
            .any(|probe| probe.probe == "process")
    );
    assert!(
        snapshot
            .discovery
            .probes
            .iter()
            .all(|probe| probe.probe != "container")
    );
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].body, "first\n");
    assert_eq!(records[1].body, "second\n");
    assert_eq!(checkpoint.files.len(), 1);
    assert_eq!(
        checkpoint.files[0].checkpoint_offset,
        "first\nsecond\n".len() as u64
    );
    assert!(!discovery_resources.is_empty());
    assert!(!discovery_targets.is_empty());
    assert!(
        discovery_resources
            .iter()
            .any(|resource| resource.kind == "host")
    );
    assert!(discovery_targets.iter().any(|target| target.kind == "host"));
    assert_eq!(discovery_meta.schema_version, "v1");
    assert_eq!(
        discovery_meta.last_success_at,
        Some(discovery_meta.generated_at.clone())
    );
    assert!(
        host_planner_candidates
            .iter()
            .all(|candidate| candidate.collection_kind == "host_metrics")
    );
    assert!(
        process_planner_candidates
            .iter()
            .all(|candidate| candidate.collection_kind == "process_metrics")
    );
    assert!(
        container_planner_candidates
            .iter()
            .all(|candidate| candidate.collection_kind == "container_metrics")
    );
    assert!(container_planner_candidates.is_empty());
    assert!(
        metrics_target_view
            .targets
            .iter()
            .any(|target| target.collection_kind == "host_metrics")
    );
    if !process_planner_candidates.is_empty() {
        assert!(
            metrics_target_view
                .targets
                .iter()
                .any(|target| target.collection_kind == "process_metrics")
        );
    }
    if !container_planner_candidates.is_empty() {
        assert!(
            metrics_target_view
                .targets
                .iter()
                .any(|target| target.collection_kind == "container_metrics")
        );
    }
    assert!(metrics_runtime_snapshot.total_targets >= metrics_runtime_snapshot.host_targets);
    assert!(metrics_runtime_snapshot.host_targets >= 1);
    assert_eq!(metrics_runtime_snapshot.container_targets, 0);
    assert_eq!(
        metrics_runtime_snapshot.total_targets,
        metrics_runtime_snapshot.host_targets
            + metrics_runtime_snapshot.process_targets
            + metrics_runtime_snapshot.container_targets
    );
    assert_eq!(
        snapshot.metrics.total_targets,
        metrics_runtime_snapshot.total_targets
    );
    assert_eq!(
        snapshot.metrics.host_targets,
        metrics_runtime_snapshot.host_targets
    );
    assert_eq!(
        snapshot.metrics.process_targets,
        metrics_runtime_snapshot.process_targets
    );
    assert_eq!(
        snapshot.metrics.container_targets,
        metrics_runtime_snapshot.container_targets
    );
    assert_eq!(
        metrics_runtime_snapshot
            .outcomes
            .iter()
            .map(|outcome| outcome.attempted_targets)
            .sum::<usize>(),
        metrics_runtime_snapshot.total_targets
    );
    assert!(
        metrics_runtime_snapshot
            .outcomes
            .iter()
            .all(|outcome| outcome.attempted_targets
                == outcome.succeeded_targets + outcome.failed_targets)
    );
    let host_outcome = metrics_runtime_snapshot
        .outcomes
        .iter()
        .find(|outcome| outcome.collection_kind == "host_metrics")
        .expect("host outcome");
    assert_eq!(host_outcome.status, "succeeded");
    assert!(host_outcome.last_error.is_none());
    assert!(!host_outcome.runtime_facts.is_empty());
    assert!(
        host_outcome
            .runtime_facts
            .iter()
            .any(|fact| fact.key == "discovery.source" || fact.key.starts_with("host."))
    );
    let process_outcome = metrics_runtime_snapshot
        .outcomes
        .iter()
        .find(|outcome| outcome.collection_kind == "process_metrics")
        .expect("process outcome");
    if metrics_runtime_snapshot.process_targets > 0 {
        assert!(matches!(
            process_outcome.status.as_str(),
            "succeeded" | "partial" | "failed"
        ));
        assert_eq!(
            process_outcome.succeeded_targets + process_outcome.failed_targets,
            process_outcome.attempted_targets
        );
        assert!(!process_outcome.runtime_facts.is_empty());
    } else {
        assert_eq!(process_outcome.status, "idle");
        assert!(process_outcome.last_error.is_none());
        assert!(process_outcome.runtime_facts.is_empty());
    }
    let container_outcome = metrics_runtime_snapshot
        .outcomes
        .iter()
        .find(|outcome| outcome.collection_kind == "container_metrics")
        .expect("container outcome");
    if metrics_runtime_snapshot.container_targets > 0 {
        assert!(matches!(
            container_outcome.status.as_str(),
            "succeeded" | "partial" | "failed"
        ));
        assert_eq!(
            container_outcome.succeeded_targets + container_outcome.failed_targets,
            container_outcome.attempted_targets
        );
        assert!(!container_outcome.runtime_facts.is_empty());
    } else {
        assert_eq!(container_outcome.status, "idle");
        assert!(container_outcome.last_error.is_none());
        assert!(container_outcome.runtime_facts.is_empty());
    }
    assert!(
        metrics_runtime_snapshot
            .outcomes
            .iter()
            .any(|outcome| outcome.collection_kind == "host_metrics")
    );
    assert!(
        metrics_runtime_snapshot
            .outcomes
            .iter()
            .flat_map(|outcome| outcome.sample_targets.iter())
            .any(|sample| !sample.candidate_id.is_empty() && !sample.target_ref.is_empty())
    );
    assert!(
        _metrics_samples_exporter.exists(),
        "metrics exporter output should exist"
    );
    assert!(snapshot.metrics.updated_at.is_some());
}

#[cfg(unix)]
#[test]
fn daemon_run_once_can_enable_high_cardinality_discovery_explicitly() {
    let root = temp_dir("daemon-explicit-discovery");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    let input_path = root.join("app.log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");
    fs::write(&input_path, "first\n").expect("write input log");

    let mut config = standalone_config_with_file_input(&root, &input_path);
    config.discovery = DiscoverySection {
        host_enabled: true,
        process_enabled: true,
        container_enabled: true,
    };

    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("daemon run once");

    let discovery_resources: Vec<DiscoveredResource> =
        read_json(&state_dir.join("discovery").join("resources.json"))
            .expect("read discovery resources");

    let process_probe = snapshot
        .discovery
        .probes
        .iter()
        .find(|probe| probe.probe == "process")
        .expect("process probe should be scheduled when explicitly enabled");
    if process_probe.status == "ok" {
        assert!(
            discovery_resources
                .iter()
                .any(|resource| resource.kind == "process")
        );
    } else {
        assert!(process_probe.error.is_some());
    }
}

#[cfg(unix)]
#[test]
fn daemon_run_once_continues_when_discovery_cache_store_fails() {
    let root = temp_dir("daemon-discovery-store-fail");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    let input_path = root.join("app.log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");
    fs::write(&input_path, "first\n").expect("write input log");

    let discovery_dir = state_dir.join("discovery");
    let config = standalone_config_with_file_input(&root, &input_path);
    daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("initial daemon run once");

    let mut perms = fs::metadata(&discovery_dir)
        .expect("discovery dir metadata")
        .permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o500);
    fs::set_permissions(&discovery_dir, perms).expect("set discovery dir readonly");

    fs::write(&input_path, "first\nsecond\n").expect("append test input");
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("daemon run once with discovery store failure");

    let output_path = root.join("log").join("warp-parse-records.ndjson");
    let output = fs::read_to_string(&output_path).expect("read output");
    let records: Vec<warp_insight_contracts::telemetry_record::TelemetryRecordContract> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse telemetry record"))
        .collect();

    assert_eq!(records.len(), 2);
    assert_eq!(snapshot.discovery.readiness, DiscoveryReadiness::Ready);
    assert!(snapshot.discovery.failure_count >= 1);
    assert!(snapshot.discovery.probes.iter().any(|probe| {
        probe.source == "cache"
            && probe.probe == "discovery"
            && probe.phase == "cache_store"
            && probe.status == "failed"
    }));
}

#[cfg(unix)]
#[test]
fn daemon_run_once_rebuilds_when_discovery_cache_is_corrupt() {
    let root = temp_dir("daemon-discovery-cache-corrupt");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    let input_path = root.join("app.log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");
    fs::write(&input_path, "first\n").expect("write input log");

    let config = standalone_config_with_file_input(&root, &input_path);
    daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("initial daemon run once");

    let discovery_dir = state_dir.join("discovery");
    fs::write(discovery_dir.join("meta.json"), "{broken-json}\n").expect("corrupt meta");
    fs::write(&input_path, "first\nsecond\n").expect("update input log");

    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("daemon run once after corrupt cache");

    let output_path = root.join("log").join("warp-parse-records.ndjson");
    let output = fs::read_to_string(&output_path).expect("read output");
    let records: Vec<warp_insight_contracts::telemetry_record::TelemetryRecordContract> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse telemetry record"))
        .collect();
    let discovery_meta: DiscoveryCacheMeta =
        read_json(&discovery_dir.join("meta.json")).expect("reloaded discovery meta");

    assert_eq!(records.len(), 2);
    assert_eq!(snapshot.discovery.readiness, DiscoveryReadiness::Ready);
    let cache_load_failures = snapshot
        .discovery
        .probes
        .iter()
        .filter(|probe| {
            probe.source == "cache"
                && probe.probe == "discovery"
                && probe.phase == "cache_load_meta"
                && probe.status == "failed"
        })
        .count();
    assert_eq!(cache_load_failures, 1);
    assert_eq!(discovery_meta.schema_version, "v1");
}

#[cfg(unix)]
#[test]
fn daemon_run_once_uses_cached_metrics_snapshot_when_target_view_is_missing() {
    let root = temp_dir("daemon-metrics-target-view-missing");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    let input_path = root.join("app.log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");
    fs::write(&input_path, "first\n").expect("write input log");

    let config = standalone_config_with_file_input(&root, &input_path);
    daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("initial daemon run once");

    let telemetry_dir = state_dir.join("telemetry");
    let target_view_path = telemetry_dir.join("metrics_target_view.json");
    let runtime_snapshot_path = telemetry_dir.join("metrics_runtime_snapshot.json");
    let cached_snapshot: TestMetricsRuntimeSnapshot =
        read_json(&runtime_snapshot_path).expect("read cached runtime snapshot");
    fs::remove_file(&target_view_path).expect("remove target view");

    let mut perms = fs::metadata(&telemetry_dir)
        .expect("telemetry dir metadata")
        .permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o500);
    fs::set_permissions(&telemetry_dir, perms).expect("set telemetry dir readonly");

    fs::write(&input_path, "first\nsecond\n").expect("append input log");
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("daemon run once with missing target view");

    let runtime_snapshot: TestMetricsRuntimeSnapshot =
        read_json(&runtime_snapshot_path).expect("read runtime snapshot after fallback");
    let output_path = root.join("log").join("warp-parse-records.ndjson");
    let output = fs::read_to_string(&output_path).expect("read output");
    let records: Vec<warp_insight_contracts::telemetry_record::TelemetryRecordContract> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse telemetry record"))
        .collect();

    assert_eq!(records.len(), 2);
    assert_eq!(
        snapshot.state,
        warp_insightd::self_observability::HealthState::Active
    );
    assert!(!snapshot.metrics.target_view_loaded);
    assert!(snapshot.metrics.used_cached_snapshot);
    assert!(snapshot.metrics.failure_count >= 1);
    assert_eq!(
        snapshot.metrics.attempted_targets,
        cached_snapshot.total_targets
    );
    assert_eq!(
        snapshot.metrics.succeeded_targets,
        cached_snapshot
            .outcomes
            .iter()
            .map(|outcome| outcome.succeeded_targets)
            .sum::<usize>()
    );
    assert_eq!(
        snapshot.metrics.failed_targets,
        cached_snapshot
            .outcomes
            .iter()
            .map(|outcome| outcome.failed_targets)
            .sum::<usize>()
    );
    assert!(
        snapshot
            .metrics
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains("target_view_load"))
    );
    assert_eq!(
        snapshot.metrics.total_targets,
        cached_snapshot.total_targets
    );
    assert_eq!(snapshot.metrics.host_targets, cached_snapshot.host_targets);
    assert_eq!(
        snapshot.metrics.process_targets,
        cached_snapshot.process_targets
    );
    assert_eq!(
        snapshot.metrics.container_targets,
        cached_snapshot.container_targets
    );
    assert_eq!(
        runtime_snapshot.total_targets,
        cached_snapshot.total_targets
    );
    assert_eq!(runtime_snapshot.host_targets, cached_snapshot.host_targets);
    assert_eq!(
        runtime_snapshot.process_targets,
        cached_snapshot.process_targets
    );
    assert_eq!(
        runtime_snapshot.container_targets,
        cached_snapshot.container_targets
    );
    assert_eq!(
        runtime_snapshot
            .outcomes
            .iter()
            .map(|outcome| outcome.succeeded_targets)
            .sum::<usize>(),
        cached_snapshot
            .outcomes
            .iter()
            .map(|outcome| outcome.succeeded_targets)
            .sum::<usize>()
    );
    assert_eq!(
        runtime_snapshot
            .outcomes
            .iter()
            .map(|outcome| outcome.failed_targets)
            .sum::<usize>(),
        cached_snapshot
            .outcomes
            .iter()
            .map(|outcome| outcome.failed_targets)
            .sum::<usize>()
    );
}

#[cfg(unix)]
#[test]
fn daemon_run_once_continues_when_one_file_input_fails() {
    let root = temp_dir("daemon-file-input-error-isolated");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    let good_input = root.join("good.log");
    let bad_input = root.join("bad-dir");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");
    fs::write(&good_input, "good\n").expect("write good input");
    fs::create_dir_all(&bad_input).expect("create bad input dir");

    let config = standalone_config_with_file_inputs(
        &root,
        vec![
            LogFileInputSection {
                input_id: "bad".to_string(),
                path: bad_input.display().to_string(),
                startup_position: "head".to_string(),
                multiline_mode: "none".to_string(),
            },
            LogFileInputSection {
                input_id: "good".to_string(),
                path: good_input.display().to_string(),
                startup_position: "head".to_string(),
                multiline_mode: "none".to_string(),
            },
        ],
    );
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("daemon run once");

    let output_path = root.join("log").join("warp-parse-records.ndjson");
    let output = fs::read_to_string(&output_path).expect("read output");
    let records: Vec<warp_insight_contracts::telemetry_record::TelemetryRecordContract> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse telemetry record"))
        .collect();
    let checkpoint_path = warp_insightd::state_store::log_checkpoints::path_for(&state_dir, "good");
    let checkpoint: TestLogCheckpointState = read_json(&checkpoint_path).expect("read checkpoint");

    assert_eq!(
        snapshot.state,
        warp_insightd::self_observability::HealthState::Active
    );
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].body, "good\n");
    assert_eq!(checkpoint.files.len(), 1);
    assert!(!warp_insightd::state_store::log_checkpoints::path_for(&state_dir, "bad").exists());
}

#[cfg(unix)]
#[test]
fn daemon_run_once_marks_active_when_only_file_input_fails() {
    let root = temp_dir("daemon-file-input-only-error");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    let bad_input = root.join("bad-dir");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");
    fs::create_dir_all(&bad_input).expect("create bad input dir");

    let config = standalone_config_with_file_inputs(
        &root,
        vec![LogFileInputSection {
            input_id: "bad".to_string(),
            path: bad_input.display().to_string(),
            startup_position: "head".to_string(),
            multiline_mode: "none".to_string(),
        }],
    );
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("daemon run once");

    assert_eq!(
        snapshot.state,
        warp_insightd::self_observability::HealthState::Active
    );
    assert!(!root.join("log").join("warp-parse-records.ndjson").exists());
}

#[cfg(unix)]
#[test]
fn daemon_run_once_marks_active_when_configured_file_is_missing() {
    let root = temp_dir("daemon-file-input-missing");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    let missing_input = root.join("missing.log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");

    let config = standalone_config_with_file_inputs(
        &root,
        vec![LogFileInputSection {
            input_id: "missing".to_string(),
            path: missing_input.display().to_string(),
            startup_position: "head".to_string(),
            multiline_mode: "none".to_string(),
        }],
    );
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("daemon run once");

    assert_eq!(
        snapshot.state,
        warp_insightd::self_observability::HealthState::Active
    );
    assert!(!root.join("log").join("warp-parse-records.ndjson").exists());
    assert!(!warp_insightd::state_store::log_checkpoints::path_for(&state_dir, "missing").exists());
}

#[cfg(unix)]
#[test]
fn daemon_run_once_replays_existing_spool_even_when_source_file_is_missing() {
    let root = temp_dir("daemon-file-input-missing-replay");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    let missing_input = root.join("missing.log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");

    let spool_path = root
        .join("state")
        .join("spool")
        .join("logs")
        .join("missing.ndjson");
    fs::create_dir_all(spool_path.parent().expect("spool dir")).expect("create spool dir");
    let first = serde_json::to_string(&TelemetryRecordContract::new_log(
        "2026-04-14T00:00:00Z".to_string(),
        "missing".to_string(),
        missing_input.display().to_string(),
        "first\n".to_string(),
        0,
        6,
    ))
    .expect("encode first");
    let second = serde_json::to_string(&TelemetryRecordContract::new_log(
        "2026-04-14T00:00:01Z".to_string(),
        "missing".to_string(),
        missing_input.display().to_string(),
        "second\n".to_string(),
        6,
        13,
    ))
    .expect("encode second");
    fs::write(&spool_path, format!("{first}\n{second}\n")).expect("write spool");

    let config = standalone_config_with_file_inputs(
        &root,
        vec![LogFileInputSection {
            input_id: "missing".to_string(),
            path: missing_input.display().to_string(),
            startup_position: "head".to_string(),
            multiline_mode: "none".to_string(),
        }],
    );
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("daemon run once");

    let output_path = root.join("log").join("warp-parse-records.ndjson");
    let output = fs::read_to_string(&output_path).expect("read output");
    let records: Vec<warp_insight_contracts::telemetry_record::TelemetryRecordContract> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse telemetry record"))
        .collect();

    assert_eq!(
        snapshot.state,
        warp_insightd::self_observability::HealthState::Active
    );
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].body, "first\n");
    assert_eq!(records[1].body, "second\n");
    assert!(!spool_path.exists());
}

#[cfg(unix)]
#[test]
fn daemon_run_once_sends_raw_log_lines_to_tcp_output() {
    let root = temp_dir("daemon-file-input-tcp");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    let input_path = root.join("app.log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");
    fs::write(&input_path, "alpha\nbeta\n").expect("write input log");

    let Some(listener) = bind_tcp_listener("127.0.0.1:0") else {
        return;
    };
    let port = listener.local_addr().expect("listener addr").port();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept");
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set timeout");
        let mut buf = Vec::new();
        let mut chunk = [0u8; 128];
        loop {
            match socket.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                Err(err)
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(err) => panic!("read tcp payload: {err}"),
            }
        }
        String::from_utf8(buf).expect("utf8 payload")
    });

    let config =
        standalone_config_with_tcp_file_input(&root, &input_path, "127.0.0.1", port, "line");
    let snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("daemon run once");

    let payload = server.join().expect("join server");
    let checkpoint_path = warp_insightd::state_store::log_checkpoints::path_for(&state_dir, "app");
    let checkpoint: TestLogCheckpointState = read_json(&checkpoint_path).expect("read checkpoint");

    assert_eq!(
        snapshot.state,
        warp_insightd::self_observability::HealthState::Active
    );
    assert_eq!(payload, "alpha\nbeta\n");
    assert_eq!(checkpoint.files.len(), 1);
    assert_eq!(
        checkpoint.files[0].checkpoint_offset,
        "alpha\nbeta\n".len() as u64
    );
    assert!(!root.join("log").join("warp-parse-records.ndjson").exists());
}

#[cfg(unix)]
#[test]
fn daemon_run_once_replays_spool_when_tcp_output_recovers() {
    let root = temp_dir("daemon-file-input-tcp-replay");
    let run_dir = root.join("run");
    let state_dir = root.join("state");
    let log_dir = root.join("log");
    let input_path = root.join("app.log");
    bootstrap::initialize(&root, &run_dir, &state_dir, &log_dir).expect("bootstrap");
    fs::write(&input_path, "first\nsecond\n").expect("write input log");

    let Some(reserved) = bind_tcp_listener("127.0.0.1:0") else {
        return;
    };
    let port = reserved.local_addr().expect("listener addr").port();
    drop(reserved);

    let failing_config =
        standalone_config_with_tcp_file_input(&root, &input_path, "127.0.0.1", port, "line");
    let first_snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &failing_config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("first daemon run once");
    let spool_path = root
        .join("state")
        .join("spool")
        .join("logs")
        .join("app.ndjson");
    let spooled = fs::read_to_string(&spool_path).expect("read spool");

    assert_eq!(
        first_snapshot.state,
        warp_insightd::self_observability::HealthState::Active
    );
    assert!(spooled.contains("\"body\":\"first\\n\""));
    assert!(spooled.contains("\"body\":\"second\\n\""));

    fs::write(&input_path, "first\nsecond\nthird\n").expect("append third line");
    let Some(listener) = bind_tcp_listener(&format!("127.0.0.1:{port}")) else {
        return;
    };
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept");
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set timeout");
        let mut buf = Vec::new();
        let mut chunk = [0u8; 128];
        loop {
            match socket.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                Err(err)
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(err) => panic!("read tcp payload: {err}"),
            }
        }
        String::from_utf8(buf).expect("utf8 payload")
    });

    let recovered_config =
        standalone_config_with_tcp_file_input(&root, &input_path, "127.0.0.1", port, "line");
    let second_snapshot = daemon::run_once(&daemon::DaemonLoop {
        config: &recovered_config,
        exec_bin: &test_exec_bin(&root),
    })
    .expect("second daemon run once");
    let payload = server.join().expect("join server");
    let checkpoint_path = warp_insightd::state_store::log_checkpoints::path_for(&state_dir, "app");
    let checkpoint: TestLogCheckpointState = read_json(&checkpoint_path).expect("read checkpoint");

    assert_eq!(
        second_snapshot.state,
        warp_insightd::self_observability::HealthState::Active
    );
    assert_eq!(payload, "first\nsecond\nthird\n");
    assert!(!spool_path.exists());
    assert_eq!(checkpoint.files.len(), 1);
    assert_eq!(
        checkpoint.files[0].checkpoint_offset,
        "first\nsecond\nthird\n".len() as u64
    );
}
