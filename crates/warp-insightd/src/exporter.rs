//! Exporter: reads internal pipeline state and writes JSONL outputs optimized
//! for downstream WarpParse rules.
//!
//! Discovery output is split by probe kind (host / process / container) so that
//! consumers can track each probe's revision independently and file sizes stay
//! proportional to each probe's data volume.
//!
//! Each JSONL line is self-contained:
//! - discovery rows carry snapshot metadata plus one resource
//! - metrics rows carry collection metadata plus one metric sample
//!
//! Process output is further split by classification (identified / named /
//! unidentified) based on has_exe / has_identity predicates. Kernel threads
//! (PID < 100) are filtered before classification. The filter is exporter-only
//! and does not affect the internal discovery cache.

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use warp_insight_contracts::exporter::ExporterSource;
use warp_insight_shared::fs::{read_json, write_bytes_atomic};
use warp_insight_shared::time::now_rfc3339;

use crate::discovery::cache as discovery_cache;
use crate::telemetry::metrics::runtime::{self as metrics_runtime, MetricsRuntimeSnapshot};
use crate::telemetry::metrics::samples;

static EXPORT_SEQ: AtomicU64 = AtomicU64::new(0);
const DISCOVERY_PROBES: &[&str] = &["host", "process", "container"];

/// Strips internal-only fields from export payloads.
fn strip_internal_fields(value: &mut serde_json::Value) {
    if let serde_json::Value::Array(arr) = value {
        for item in arr.iter_mut() {
            if let serde_json::Value::Object(obj) = item {
                obj.remove("origin_idx");
            }
        }
    }
}

/// Predicates for process classification.
fn has_exe(r: &serde_json::Value) -> bool {
    r.get("attributes")
        .and_then(|a| a.get("process.executable.name"))
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.is_empty())
}

fn has_identity(r: &serde_json::Value) -> bool {
    r.get("attributes")
        .and_then(|a| a.get("process.identity"))
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.is_empty())
}

fn is_kernel_thread(r: &serde_json::Value) -> bool {
    r.get("attributes")
        .and_then(|a| a.get("process.pid"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .map(|pid| pid < 100)
        .unwrap_or(false)
}

enum ExportResult {
    Written,
    Skipped,
}

fn filter_by_kind(arr: &serde_json::Value, kind: &str) -> Vec<serde_json::Value> {
    arr.as_array()
        .map(|items| {
            items
                .iter()
                .filter(|item| item.get("kind").and_then(|v| v.as_str()) == Some(kind))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn snapshot_fields(meta: Option<&serde_json::Value>) -> (String, u64, String) {
    (
        meta.and_then(|m| m.get("snapshot_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        meta.and_then(|m| m.get("revision"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        meta.and_then(|m| m.get("generated_at"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    )
}

fn matching_target(targets: &[serde_json::Value], resource_id: &str) -> Option<serde_json::Value> {
    targets
        .iter()
        .find(|target| {
            target
                .get("resource_ref")
                .and_then(|value| value.as_str())
                == Some(resource_id)
        })
        .cloned()
}

fn write_jsonl(path: &Path, rows: &[serde_json::Value]) -> io::Result<()> {
    let mut buf = String::new();
    for (idx, row) in rows.iter().enumerate() {
        if idx > 0 {
            buf.push('\n');
        }
        let line = serde_json::to_string(row).map_err(io::Error::other)?;
        buf.push_str(&line);
    }
    write_bytes_atomic(path, buf.as_bytes())
}

fn build_disc_row(
    source: &ExporterSource,
    probe: &str,
    classification: Option<&str>,
    output_id: &str,
    seq: u64,
    generated_at: &str,
    snapshot_id: &str,
    snapshot_revision: u64,
    snapshot_generated_at: &str,
    resource: serde_json::Value,
    target: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut row = serde_json::Map::new();
    row.insert(
        "api_version".to_string(),
        serde_json::Value::String("warp-insight/v1".to_string()),
    );
    row.insert(
        "kind".to_string(),
        serde_json::Value::String("disc_resource".to_string()),
    );
    row.insert(
        "output_id".to_string(),
        serde_json::Value::String(output_id.to_string()),
    );
    row.insert("seq".to_string(), serde_json::Value::Number(seq.into()));
    row.insert(
        "generated_at".to_string(),
        serde_json::Value::String(generated_at.to_string()),
    );
    row.insert(
        "source".to_string(),
        serde_json::to_value(source.clone().with_probe(probe)).expect("serialize source"),
    );
    row.insert(
        "snapshot_id".to_string(),
        serde_json::Value::String(snapshot_id.to_string()),
    );
    row.insert(
        "snapshot_revision".to_string(),
        serde_json::Value::Number(snapshot_revision.into()),
    );
    row.insert(
        "snapshot_generated_at".to_string(),
        serde_json::Value::String(snapshot_generated_at.to_string()),
    );
    if let Some(classification) = classification {
        row.insert(
            "classification".to_string(),
            serde_json::Value::String(classification.to_string()),
        );
    }
    row.insert("resource".to_string(), resource);
    if let Some(target) = target {
        row.insert("target".to_string(), target);
    }
    serde_json::Value::Object(row)
}

/// Writes one per-probe discovery snapshot file (JSONL format).
/// Used for host and container.
fn export_probe(
    state_dir: &Path,
    source: &ExporterSource,
    probe: &str,
    resources: &serde_json::Value,
    targets: &serde_json::Value,
    meta: Option<&serde_json::Value>,
) -> io::Result<ExportResult> {
    let probe_resources = filter_by_kind(resources, probe);
    if probe_resources.is_empty() {
        return Ok(ExportResult::Skipped);
    }
    let probe_targets = filter_by_kind(targets, probe);
    let (snapshot_id, snapshot_revision, snapshot_generated_at) = snapshot_fields(meta);
    let seq = EXPORT_SEQ.fetch_add(1, Ordering::Relaxed);
    let output_id = format!("{probe}_{seq}");
    let generated_at = now_rfc3339();

    let rows: Vec<_> = probe_resources
        .into_iter()
        .map(|resource| {
            let resource_id = resource
                .get("resource_id")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            build_disc_row(
                source,
                probe,
                None,
                &output_id,
                seq,
                &generated_at,
                &snapshot_id,
                snapshot_revision,
                &snapshot_generated_at,
                resource,
                matching_target(&probe_targets, &resource_id),
            )
        })
        .collect();

    let out_path = state_dir.join("export").join(format!("{probe}.jsonl"));
    write_jsonl(&out_path, &rows)?;
    Ok(ExportResult::Written)
}

/// Reads discovery cache and writes one file per probe kind.
pub fn export_disc_snap(state_dir: &Path, source: &ExporterSource) -> io::Result<()> {
    let paths = discovery_cache::DiscoveryCachePaths::under_state_dir(state_dir);

    let mut resources = match read_json::<serde_json::Value>(&paths.resources) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("exporter: discovery cache skipped: {err}");
            return Ok(());
        }
    };
    let mut targets = read_json::<serde_json::Value>(&paths.targets).unwrap_or_default();
    strip_internal_fields(&mut resources);
    strip_internal_fields(&mut targets);
    let meta = read_json(&paths.meta).ok();

    for probe in DISCOVERY_PROBES {
        if *probe == "process" {
            if let Err(err) =
                export_process_classified(state_dir, source, &resources, &targets, meta.as_ref())
            {
                eprintln!("exporter: process_classified error: {err}");
            }
        } else if let Err(err) =
            export_probe(state_dir, source, probe, &resources, &targets, meta.as_ref())
        {
            eprintln!("exporter: {probe} export error: {err}");
        }
    }
    Ok(())
}

/// Classifies process resources and writes one JSONL file per set.
fn export_process_classified(
    state_dir: &Path,
    source: &ExporterSource,
    resources: &serde_json::Value,
    targets: &serde_json::Value,
    meta: Option<&serde_json::Value>,
) -> io::Result<()> {
    let mut identified = Vec::new();
    let mut named = Vec::new();
    let mut unidentified = Vec::new();
    let process_targets = filter_by_kind(targets, "process");

    if let Some(items) = resources.as_array() {
        for item in items {
            if item.get("kind").and_then(|v| v.as_str()) != Some("process") {
                continue;
            }
            if is_kernel_thread(item) {
                continue;
            }
            match (has_exe(item), has_identity(item)) {
                (true, true) => identified.push(item.clone()),
                (true, false) => named.push(item.clone()),
                (false, _) => unidentified.push(item.clone()),
            }
        }
    }

    let sets: [(&str, Vec<serde_json::Value>); 3] = [
        ("identified", identified),
        ("named", named),
        ("unidentified", unidentified),
    ];
    let (snapshot_id, snapshot_revision, snapshot_generated_at) = snapshot_fields(meta);

    for (set_name, proc_resources) in sets {
        if proc_resources.is_empty() {
            continue;
        }
        let seq = EXPORT_SEQ.fetch_add(1, Ordering::Relaxed);
        let output_id = format!("process_{set_name}_{seq}");
        let generated_at = now_rfc3339();
        let rows: Vec<_> = proc_resources
            .into_iter()
            .map(|resource| {
                let resource_id = resource
                    .get("resource_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                build_disc_row(
                    source,
                    "process",
                    Some(set_name),
                    &output_id,
                    seq,
                    &generated_at,
                    &snapshot_id,
                    snapshot_revision,
                    &snapshot_generated_at,
                    resource,
                    matching_target(&process_targets, &resource_id),
                )
            })
            .collect();

        let out_path = state_dir.join("export").join(format!("process-{set_name}.jsonl"));
        write_jsonl(&out_path, &rows)?;
    }
    Ok(())
}

/// Reads current metrics runtime snapshot and writes one sample per JSONL line.
pub fn export_metrics(state_dir: &Path, source: &ExporterSource) -> io::Result<()> {
    let runtime_path = metrics_runtime::path_for(state_dir);

    match read_json::<MetricsRuntimeSnapshot>(&runtime_path) {
        Ok(snapshot) => {
            let samples_snapshot = samples::build_samples_snapshot(&snapshot);
            let seq = EXPORT_SEQ.fetch_add(1, Ordering::Relaxed);
            let output_id = format!("metrics_{seq}");
            let generated_at = now_rfc3339();
            let mut rows = Vec::new();

            for group in &samples_snapshot.groups {
                for sample in &group.samples {
                    rows.push(serde_json::json!({
                        "api_version": "warp-insight/v1",
                        "kind": "metrics_sample",
                        "output_id": output_id,
                        "seq": seq,
                        "generated_at": generated_at,
                        "source": source,
                        "batch_seq": samples_snapshot.batch_seq,
                        "collected_at": samples_snapshot.collected_at,
                        "collection_kind": group.kind,
                        "target_ref": group.target_ref,
                        "resource_ref": group.resource_ref,
                        "name": sample.name,
                        "type": sample.value_type,
                        "unit": sample.unit,
                        "value": sample.value,
                        "status": sample.status,
                    }));
                }
            }

            let out_path = state_dir.join("export").join("metrics.jsonl");
            write_jsonl(&out_path, &rows)
        }
        Err(err) => {
            eprintln!("exporter: metrics skipped (no runtime snapshot): {err}");
            Ok(())
        }
    }
}

/// Export all probe discovery snapshots and metrics. Errors are logged, not propagated.
pub fn export_all(state_dir: &Path, source: &ExporterSource) {
    if let Err(err) = export_disc_snap(state_dir, source) {
        eprintln!("exporter: disc_snap error: {err}");
    }
    if let Err(err) = export_metrics(state_dir, source) {
        eprintln!("exporter: metrics error: {err}");
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::BufRead;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use warp_insight_shared::fs::read_json;

    use super::*;
    use crate::discovery::cache as discovery_cache;

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
        fs::write(&paths.resources, serde_json::to_vec_pretty(&resources).unwrap())
            .expect("write resources");
        fs::write(&paths.targets, serde_json::to_vec_pretty(&targets).unwrap())
            .expect("write targets");
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
        assert!(cached[0].get("origin_idx").is_some(), "cache keeps origin_idx");

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
}
