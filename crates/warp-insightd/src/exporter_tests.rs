use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use warp_insight_contracts::exporter::ExporterSource;
use warp_insight_shared::fs::read_json;

use crate::discovery::cache as discovery_cache;
use crate::exporter::{EXPORT_SEQ, export_disc_snap, export_metrics};
use crate::telemetry::metrics::runtime::{self as metrics_runtime, MetricsRuntimeSnapshot};

fn temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("duration")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("warp-insight-exporter-{name}-{suffix}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_cache(state_dir: &Path, resources: serde_json::Value, targets: serde_json::Value) {
    let paths = discovery_cache::DiscoveryCachePaths::under_state_dir(state_dir);
    fs::create_dir_all(&paths.root).expect("create discovery dir");
    let meta = serde_json::json!({
        "schema_version": "v1",
        "snapshot_id": "snap-1",
        "revision": 1,
        "generated_at": "2026-04-19T00:00:00Z",
        "origins": [],
    });
    fs::write(
        &paths.resources,
        serde_json::to_vec_pretty(&resources).unwrap(),
    )
    .expect("write resources");
    fs::write(&paths.targets, serde_json::to_vec_pretty(&targets).unwrap()).expect("write targets");
    fs::write(&paths.meta, serde_json::to_vec_pretty(&meta).unwrap()).expect("write meta");
}

fn read_jsonl(path: &Path) -> Vec<serde_json::Value> {
    std::io::BufReader::new(fs::File::open(path).expect("open jsonl"))
        .lines()
        .map(|line| serde_json::from_str(&line.expect("line")).expect("json line"))
        .collect()
}

#[test]
fn export_host_writes_host_resources_only() {
    let state_dir = temp_dir("host-only");
    write_cache(
        &state_dir,
        serde_json::json!([
            {"resource_id":"h1","kind":"host","attributes":{"a":"1"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"host"},
            {"resource_id":"p1","kind":"process","attributes":{"b":"2"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"process"}
        ]),
        serde_json::json!([
            {"target_id":"h1:host","kind":"host","resource_ref":"h1","execution_hints":{},"state":"active"}
        ]),
    );
    let source = ExporterSource::new("a", "i");
    export_disc_snap(&state_dir, &source).expect("export");

    let host_rows = read_jsonl(&state_dir.join("export").join("host.jsonl"));
    assert_eq!(host_rows.len(), 1);
    assert_eq!(host_rows[0]["kind"], "disc_resource");
    assert_eq!(host_rows[0]["source"]["probe"], "host");
    assert_eq!(host_rows[0]["snapshot_id"], "snap-1");
    assert_eq!(host_rows[0]["snapshot_revision"], 1);
    assert_eq!(host_rows[0]["resource"]["resource_id"], "h1");
    assert_eq!(host_rows[0]["target"]["target_id"], "h1:host");

    let path = state_dir.join("export").join("process-unidentified.jsonl");
    assert!(path.exists());
    let proc_rows = read_jsonl(&path);
    assert_eq!(proc_rows.len(), 1);
    assert_eq!(proc_rows[0]["classification"], "unidentified");

    assert!(!state_dir.join("export").join("container.jsonl").exists());
}

#[test]
fn process_classification_produces_three_sets() {
    let state_dir = temp_dir("proc-class");
    write_cache(
        &state_dir,
        serde_json::json!([
            {"resource_id":"p1","kind":"process","attributes":{"process.pid":"100","process.executable.name":"nginx","process.identity":"abc"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"process"},
            {"resource_id":"p2","kind":"process","attributes":{"process.pid":"200","process.executable.name":"bash"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"process"},
            {"resource_id":"p3","kind":"process","attributes":{"process.pid":"300"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"process"},
            {"resource_id":"h1","kind":"host","attributes":{"host.name":"demo"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"host"}
        ]),
        serde_json::json!([
            {"target_id":"p1:process","kind":"process","resource_ref":"p1","execution_hints":{},"state":"active"},
            {"target_id":"p2:process","kind":"process","resource_ref":"p2","execution_hints":{},"state":"active"},
            {"target_id":"p3:process","kind":"process","resource_ref":"p3","execution_hints":{},"state":"active"}
        ]),
    );
    let source = ExporterSource::new("a", "b");
    export_disc_snap(&state_dir, &source).expect("export");

    let dir = state_dir.join("export");
    for (name, classification) in &[
        ("process-identified.jsonl", "identified"),
        ("process-named.jsonl", "named"),
        ("process-unidentified.jsonl", "unidentified"),
    ] {
        let rows = read_jsonl(&dir.join(name));
        assert_eq!(rows.len(), 1, "{name} should have 1 resource row");
        assert_eq!(rows[0]["source"]["probe"], "process");
        assert_eq!(rows[0]["classification"], *classification);
    }
    assert!(!dir.join("process.json").exists());

    let host_rows = read_jsonl(&dir.join("host.jsonl"));
    assert_eq!(host_rows.len(), 1);
    assert_eq!(host_rows[0]["resource"]["resource_id"], "h1");
}

#[test]
fn export_disc_snap_sets_probe_on_source() {
    let state_dir = temp_dir("probe");
    write_cache(
        &state_dir,
        serde_json::json!([
            {"resource_id":"h1","kind":"host","attributes":{},"discovered_at":"","last_seen_at":"","health":"healthy","source":"host"}
        ]),
        serde_json::json!([
            {"target_id":"h1:host","kind":"host","resource_ref":"h1","execution_hints":{},"state":"active"}
        ]),
    );
    let source = ExporterSource::new("a", "i");
    export_disc_snap(&state_dir, &source).expect("export");

    let host_rows = read_jsonl(&state_dir.join("export").join("host.jsonl"));
    assert_eq!(host_rows[0]["kind"], "disc_resource");
    assert_eq!(host_rows[0]["source"]["probe"], "host");
}

#[test]
fn export_network_inventory_writes_network_probe_files() {
    let state_dir = temp_dir("network");
    write_cache(
        &state_dir,
        serde_json::json!([
            {"resource_id":"h1:if:en0","kind":"network_interface","attributes":{"net.if.name":"en0"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"network"},
            {"resource_id":"h1:if:en0:ip:3139322e3136382e33312e3130","kind":"ip_address","attributes":{"net.if.addr":"192.168.31.10","net.if.prefix":"24"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"network"}
        ]),
        serde_json::json!([
            {"target_id":"h1:if:en0:network_interface","kind":"network_interface","resource_ref":"h1:if:en0","execution_hints":{},"state":"active"},
            {"target_id":"h1:if:en0:ip:3139322e3136382e33312e3130:ip_address","kind":"ip_address","resource_ref":"h1:if:en0:ip:3139322e3136382e33312e3130","execution_hints":{},"state":"active"}
        ]),
    );
    let source = ExporterSource::new("a", "i");
    export_disc_snap(&state_dir, &source).expect("export");

    let dir = state_dir.join("export");
    let iface_rows = read_jsonl(&dir.join("network_interface.jsonl"));
    assert_eq!(iface_rows.len(), 1);
    assert_eq!(iface_rows[0]["source"]["probe"], "network_interface");
    assert_eq!(iface_rows[0]["resource"]["resource_id"], "h1:if:en0");
    assert_eq!(
        iface_rows[0]["target"]["target_id"],
        "h1:if:en0:network_interface"
    );

    let address_rows = read_jsonl(&dir.join("ip_address.jsonl"));
    assert_eq!(address_rows.len(), 1);
    assert_eq!(address_rows[0]["source"]["probe"], "ip_address");
    assert_eq!(
        address_rows[0]["resource"]["attributes"]["net.if.addr"],
        "192.168.31.10"
    );
}

#[test]
fn export_endpoint_inventory_writes_service_endpoint_probe_file() {
    let state_dir = temp_dir("endpoint");
    write_cache(
        &state_dir,
        serde_json::json!([
            {"resource_id":"h1:endpoint:tcp:3132372e302e302e31:8080:12345","kind":"service_endpoint","attributes":{"endpoint.protocol":"tcp","endpoint.bind.ip":"127.0.0.1","endpoint.bind.port":"8080","socket.inode":"12345","process.pid":"42","process.identity":"linux_proc_start:99","process.ref":"h1:pid:42:linux_proc_start:99","runtime.binding.evidence":"socket_inode_owner","cgroup.path":"/kubepods/test","container.id":"container-a","container.ref":"container-a","runtime.binding.container.evidence":"process_cgroup"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"endpoint"}
        ]),
        serde_json::json!([
            {"target_id":"h1:endpoint:tcp:3132372e302e302e31:8080:12345:service_endpoint","kind":"service_endpoint","resource_ref":"h1:endpoint:tcp:3132372e302e302e31:8080:12345","execution_hints":{"process.pid":"42","process.identity":"linux_proc_start:99","process.ref":"h1:pid:42:linux_proc_start:99","container.ref":"container-a"},"state":"active"}
        ]),
    );
    let source = ExporterSource::new("a", "i");
    export_disc_snap(&state_dir, &source).expect("export");

    let rows = read_jsonl(&state_dir.join("export").join("service_endpoint.jsonl"));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["source"]["probe"], "service_endpoint");
    assert_eq!(
        rows[0]["resource"]["attributes"]["endpoint.bind.port"],
        "8080"
    );
    assert_eq!(
        rows[0]["resource"]["attributes"]["runtime.binding.evidence"],
        "socket_inode_owner"
    );
    assert_eq!(
        rows[0]["resource"]["attributes"]["process.ref"],
        "h1:pid:42:linux_proc_start:99"
    );
    assert_eq!(
        rows[0]["resource"]["attributes"]["container.ref"],
        "container-a"
    );
    assert_eq!(
        rows[0]["target"]["target_id"],
        "h1:endpoint:tcp:3132372e302e302e31:8080:12345:service_endpoint"
    );
}

#[test]
fn export_metrics_writes_flattened_samples() {
    let state_dir = temp_dir("metrics");
    fs::create_dir_all(state_dir.join("telemetry")).expect("create telemetry dir");

    use crate::telemetry::metrics::runtime::{
        MetricsCollectionOutcome, MetricsCollectionTargetSample,
    };
    use warp_insight_contracts::discovery::StringKeyValue;

    let outcome = MetricsCollectionOutcome {
        collection_kind: "host_metrics".to_string(),
        status: "succeeded".to_string(),
        attempted_targets: 1,
        succeeded_targets: 1,
        failed_targets: 0,
        last_error: None,
        runtime_facts: vec![],
        sample_targets: vec![MetricsCollectionTargetSample {
            candidate_id: "host-1".to_string(),
            target_ref: "host-1:host".to_string(),
            status: "succeeded".to_string(),
            last_error: None,
            resource_ref: "host-1".to_string(),
            execution_hints: vec![],
            runtime_facts: vec![StringKeyValue::new("host.uptime.seconds", "42")],
        }],
    };
    let runtime_snapshot = MetricsRuntimeSnapshot {
        generated_at: "2026-04-19T00:00:00Z".to_string(),
        total_targets: 1,
        host_targets: 1,
        process_targets: 0,
        container_targets: 0,
        outcomes: vec![outcome],
    };
    metrics_runtime::store(&metrics_runtime::path_for(&state_dir), &runtime_snapshot)
        .expect("store runtime snapshot");

    let source = ExporterSource::new("test-agent", "test-instance");
    export_metrics(&state_dir, &source).expect("export metrics");

    let rows = read_jsonl(&state_dir.join("export").join("metrics.jsonl"));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["kind"], "metrics_sample");
    assert_eq!(rows[0]["collected_at"], "2026-04-19T00:00:00Z");
    assert_eq!(rows[0]["target_ref"], "host-1:host");
    assert_eq!(rows[0]["name"], "system.uptime");
    assert_eq!(rows[0]["value"], 42.0);
    assert_eq!(rows[0]["source"]["agent_id"], "test-agent");
}

#[test]
fn export_skips_when_no_cache() {
    let state_dir = temp_dir("no-cache");
    let source = ExporterSource::new("a", "b");
    assert!(export_disc_snap(&state_dir, &source).is_ok());
    assert!(export_metrics(&state_dir, &source).is_ok());
}

#[test]
fn seq_increases_per_export_call() {
    const BASE: u64 = 10000;
    EXPORT_SEQ.store(BASE, Ordering::Relaxed);
    let state_dir = temp_dir("seq");
    write_cache(
        &state_dir,
        serde_json::json!([
            {"resource_id":"h1","kind":"host","attributes":{},"discovered_at":"","last_seen_at":"","health":"healthy","source":"host"}
        ]),
        serde_json::json!([
            {"target_id":"h1:host","kind":"host","resource_ref":"h1","execution_hints":{},"state":"active"}
        ]),
    );
    let source = ExporterSource::new("a", "i");

    export_disc_snap(&state_dir, &source).expect("first");
    let first = read_jsonl(&state_dir.join("export").join("host.jsonl"));
    export_disc_snap(&state_dir, &source).expect("second");
    let second = read_jsonl(&state_dir.join("export").join("host.jsonl"));

    assert!(
        second[0]["seq"].as_u64().unwrap() > first[0]["seq"].as_u64().unwrap(),
        "seq must increase"
    );
    assert!(first[0]["seq"].as_u64().unwrap() >= BASE);
}

#[test]
fn strips_origin_idx_from_export_not_cache() {
    let state_dir = temp_dir("strip");
    write_cache(
        &state_dir,
        serde_json::json!([
            {"resource_id":"h1","kind":"host","origin_idx":5,"attributes":{},"discovered_at":"","last_seen_at":"","health":"healthy","source":"host"}
        ]),
        serde_json::json!([
            {"target_id":"h1:host","kind":"host","origin_idx":5,"resource_ref":"h1","execution_hints":{},"state":"active"}
        ]),
    );
    let source = ExporterSource::new("a", "b");
    export_disc_snap(&state_dir, &source).expect("export");

    let cached: serde_json::Value =
        read_json(&discovery_cache::DiscoveryCachePaths::under_state_dir(&state_dir).resources)
            .expect("cache");
    assert!(
        cached[0].get("origin_idx").is_some(),
        "cache keeps origin_idx"
    );

    let rows = read_jsonl(&state_dir.join("export").join("host.jsonl"));
    assert!(
        rows[0]["resource"].get("origin_idx").is_none(),
        "export strips origin_idx"
    );
}

#[test]
fn per_probe_files_contain_only_matching_kind() {
    let state_dir = temp_dir("per-probe");
    let paths = discovery_cache::DiscoveryCachePaths::under_state_dir(&state_dir);
    fs::create_dir_all(&paths.root).expect("create dir");

    let resources = serde_json::json!([
        {"resource_id":"h1","kind":"host","attributes":{},"discovered_at":"","last_seen_at":"","health":"healthy","source":"host"},
        {"resource_id":"p1","kind":"process","attributes":{},"discovered_at":"","last_seen_at":"","health":"healthy","source":"process"}
    ]);
    let meta = serde_json::json!({
        "schema_version": "v1",
        "snapshot_id": "s",
        "revision": 1,
        "generated_at": "",
        "origins": []
    });
    fs::write(&paths.resources, resources.to_string()).expect("write");
    fs::write(&paths.meta, meta.to_string()).expect("write");
    fs::write(&paths.targets, "[]").expect("write");

    export_disc_snap(&state_dir, &ExporterSource::new("a", "b")).expect("export");

    let host_rows = read_jsonl(&state_dir.join("export").join("host.jsonl"));
    assert_eq!(host_rows.len(), 1);
    assert_eq!(host_rows[0]["source"]["probe"], "host");
    assert_eq!(host_rows[0]["resource"]["resource_id"], "h1");

    let proc_rows = read_jsonl(&state_dir.join("export").join("process-unidentified.jsonl"));
    assert_eq!(proc_rows.len(), 1);
    assert_eq!(proc_rows[0]["classification"], "unidentified");
}
