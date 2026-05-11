//! Exporter: reads internal pipeline state and writes unified envelope-wrapped output.
//!
//! Discovery output is split by probe kind (host / process / container) so that
//! consumers can track each probe's revision independently and file sizes stay
//! proportional to each probe's data volume.
//!
//! Process output is further split by classification (identified / named /
//! unidentified) based on has_exe / has_identity predicates.  Kernel threads
//! (PID < 100) are filtered before classification.  The filter is exporter-
//! only and does not affect the internal discovery cache.
//!
//! Process files use JSON Lines format (.jsonl): first line is the envelope
//! header, subsequent lines are one compact resource each.

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use warp_insight_contracts::exporter::{ExporterOutput, ExporterSource};
use warp_insight_shared::fs::{read_json, write_json_compact_atomic};
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

/// Writes one per-probe discovery snapshot file. Used for host and container.
fn export_probe(
    state_dir: &Path,
    source: &ExporterSource,
    probe: &str,
    resources: &serde_json::Value,
    targets: &serde_json::Value,
    meta: Option<&serde_json::Value>,
) -> io::Result<ExportResult> {
    let filter = |arr: &serde_json::Value| -> Vec<serde_json::Value> {
        arr.as_array()
            .map(|items| {
                items
                    .iter()
                    .filter(|item| item.get("kind").and_then(|v| v.as_str()) == Some(probe))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    };

    let probe_resources = filter(resources);
    if probe_resources.is_empty() {
        return Ok(ExportResult::Skipped);
    }
    let probe_targets = filter(targets);

    let seq = EXPORT_SEQ.fetch_add(1, Ordering::Relaxed);
    let output_id = format!("{probe}_{seq}");
    let snapshot = serde_json::json!({
        "id": meta.and_then(|m| m.get("snapshot_id")).and_then(|v| v.as_str()).unwrap_or("unknown"),
        "revision": meta.and_then(|m| m.get("revision")).and_then(|v| v.as_u64()).unwrap_or(0),
        "generated_at": meta.and_then(|m| m.get("generated_at")).and_then(|v| v.as_str()).unwrap_or(""),
    });

    let payload = serde_json::json!({
        "snapshot": snapshot,
        "resources": probe_resources,
        "targets": probe_targets,
    });

    let output = ExporterOutput::new(
        "disc_snap",
        output_id,
        seq,
        now_rfc3339(),
        source.clone().with_probe(probe),
        payload,
    );

    let out_path = state_dir.join("export").join(format!("{probe}.json"));
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_json_compact_atomic(&out_path, &output)?;
    Ok(ExportResult::Written)
}

/// Reads discovery cache and writes one envelope-wrapped file per probe kind.
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
            if let Err(err) = export_process_classified(state_dir, source, &resources, &targets, meta.as_ref()) {
                eprintln!("exporter: process_classified error: {err}");
            }
        } else {
            if let Err(err) = export_probe(state_dir, source, probe, &resources, &targets, meta.as_ref()) {
                eprintln!("exporter: {probe} export error: {err}");
            }
        }
    }
    Ok(())
}

/// Classifies process resources, writes one JSON Lines file per set.
/// First line is the envelope header; subsequent lines are one resource each.
fn export_process_classified(
    state_dir: &Path,
    source: &ExporterSource,
    resources: &serde_json::Value,
    _targets: &serde_json::Value,
    meta: Option<&serde_json::Value>,
) -> io::Result<()> {
    let mut identified = Vec::new();
    let mut named = Vec::new();
    let mut unidentified = Vec::new();

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

    let sets: &[(&str, Vec<serde_json::Value>)] = &[
        ("identified", identified),
        ("named", named),
        ("unidentified", unidentified),
    ];

    for (set_name, proc_resources) in sets {
        if proc_resources.is_empty() {
            continue;
        }
        let seq = EXPORT_SEQ.fetch_add(1, Ordering::Relaxed);
        let output_id = format!("process_{set_name}_{seq}");
        let snapshot = serde_json::json!({
            "id": meta.and_then(|m| m.get("snapshot_id")).and_then(|v| v.as_str()).unwrap_or("unknown"),
            "revision": meta.and_then(|m| m.get("revision")).and_then(|v| v.as_u64()).unwrap_or(0),
            "generated_at": meta.and_then(|m| m.get("generated_at")).and_then(|v| v.as_str()).unwrap_or(""),
        });

        // Header line: envelope + snapshot metadata, no resources array
        let header = serde_json::json!({
            "api_version": "warp-insight/v1",
            "kind": "disc_snap",
            "output_id": output_id,
            "seq": seq,
            "generated_at": now_rfc3339(),
            "source": {
                "agent_id": source.agent_id,
                "instance_id": source.instance_id,
                "probe": set_name,
            },
            "payload": {
                "snapshot": snapshot,
            },
        });

        let out_path = state_dir.join("export").join(format!("process-{set_name}.jsonl"));
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = serde_json::to_string(&header).map_err(io::Error::other)?;
        buf.push('\n');
        for resource in proc_resources {
            let line = serde_json::to_string(resource).map_err(io::Error::other)?;
            buf.push_str(&line);
            buf.push('\n');
        }
        std::fs::write(&out_path, buf.as_bytes())?;
    }
    Ok(())
}

/// Reads current metrics runtime snapshot, builds grouped samples, and writes
/// envelope-wrapped metrics output.
pub fn export_metrics(state_dir: &Path, source: &ExporterSource) -> io::Result<()> {
    let runtime_path = metrics_runtime::path_for(state_dir);

    match read_json::<MetricsRuntimeSnapshot>(&runtime_path) {
        Ok(snapshot) => {
            let samples_snapshot = samples::build_samples_snapshot(&snapshot);
            let seq = EXPORT_SEQ.fetch_add(1, Ordering::Relaxed);
            let output_id = format!("metrics_{seq}");

            let payload = serde_json::json!({
                "batch_seq": samples_snapshot.batch_seq,
                "collected_at": samples_snapshot.collected_at,
                "groups": samples_snapshot.groups,
            });

            let output = ExporterOutput::new(
                "metrics",
                output_id,
                seq,
                now_rfc3339(),
                source.clone(),
                payload,
            );

            let out_path = state_dir.join("export").join("metrics.json");
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            write_json_compact_atomic(&out_path, &output)
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
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use warp_insight_contracts::exporter::ExporterOutput;
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

    fn write_cache(
        state_dir: &Path,
        resources: serde_json::Value,
        targets: serde_json::Value,
    ) {
        let paths = discovery_cache::DiscoveryCachePaths::under_state_dir(state_dir);
        fs::create_dir_all(&paths.root).expect("create discovery dir");
        let meta = serde_json::json!({
            "schema_version": "v1", "snapshot_id": "snap-1", "revision": 1,
            "generated_at": "2026-04-19T00:00:00Z", "origins": [],
        });
        fs::write(&paths.resources, serde_json::to_vec_pretty(&resources).unwrap()).expect("write resources");
        fs::write(&paths.targets, serde_json::to_vec_pretty(&targets).unwrap()).expect("write targets");
        fs::write(&paths.meta, serde_json::to_vec_pretty(&meta).unwrap()).expect("write meta");
    }

    fn read_export(state_dir: &Path, name: &str) -> ExporterOutput<serde_json::Value> {
        read_json(&state_dir.join("export").join(name)).expect("read export")
    }

    fn count_jsonl(path: &Path) -> io::Result<usize> {
        let content = std::fs::read_to_string(path)?;
        Ok(content.lines().filter(|l| !l.trim().is_empty()).count())
    }

    #[test]
    fn export_host_writes_host_resources_only() {
        let state_dir = temp_dir("host-only");
        write_cache(&state_dir,
            serde_json::json!([
                {"resource_id":"h1","kind":"host","attributes":{"a":"1"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"host"},
                {"resource_id":"p1","kind":"process","attributes":{"b":"2"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"process"},
            ]),
            serde_json::json!([
                {"target_id":"h1:host","kind":"host","resource_ref":"h1","execution_hints":{},"state":"active"},
            ]),
        );
        let source = ExporterSource::new("a", "i");
        export_disc_snap(&state_dir, &source).expect("export");

        let host = read_export(&state_dir, "host.json");
        assert_eq!(host.payload["resources"].as_array().unwrap().len(), 1);
        assert_eq!(host.payload["resources"][0]["resource_id"], "h1");
        assert_eq!(host.source.probe.as_deref(), Some("host"));

        // process-unidentified.jsonl (process has no exe/identity)
        let path = state_dir.join("export").join("process-unidentified.jsonl");
        assert!(path.exists());
        let lines = count_jsonl(&path).expect("count lines");
        assert!(lines >= 2); // header + 1 resource
        assert_eq!(lines, 2);

        // container.json should NOT exist (no container data)
        assert!(!state_dir.join("export").join("container.json").exists());
    }

    #[test]
    fn process_classification_produces_three_sets() {
        let state_dir = temp_dir("proc-class");
        write_cache(&state_dir,
            serde_json::json!([
                {"resource_id":"p1","kind":"process","attributes":{"process.pid":"100","process.executable.name":"nginx","process.identity":"abc"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"process"},
                {"resource_id":"p2","kind":"process","attributes":{"process.pid":"200","process.executable.name":"bash"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"process"},
                {"resource_id":"p3","kind":"process","attributes":{"process.pid":"300"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"process"},
                {"resource_id":"h1","kind":"host","attributes":{"host.name":"demo"},"discovered_at":"","last_seen_at":"","health":"healthy","source":"host"},
            ]),
            serde_json::json!([]),
        );
        let source = ExporterSource::new("a", "b");
        export_disc_snap(&state_dir, &source).expect("export");

        let dir = state_dir.join("export");
        for name in &["process-identified.jsonl", "process-named.jsonl", "process-unidentified.jsonl"] {
            assert!(dir.join(name).exists(), "{name} should exist");
        }
        assert!(!dir.join("process.json").exists());

        // Each file has header + 1 resource = 2 lines
        for name in &["process-identified.jsonl", "process-named.jsonl", "process-unidentified.jsonl"] {
            let lines = count_jsonl(&dir.join(name)).expect("count");
            assert_eq!(lines, 2, "{name} should have header + 1 resource");
        }

        // host.json unchanged
        let host: ExporterOutput<serde_json::Value> = read_json(&dir.join("host.json")).expect("host");
        assert_eq!(host.payload["resources"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn export_disc_snap_sets_probe_on_source() {
        let state_dir = temp_dir("probe");
        write_cache(&state_dir,
            serde_json::json!([{"resource_id":"h1","kind":"host","attributes":{},"discovered_at":"","last_seen_at":"","health":"healthy","source":"host"}]),
            serde_json::json!([{"target_id":"h1:host","kind":"host","resource_ref":"h1","execution_hints":{},"state":"active"}]),
        );
        let source = ExporterSource::new("a", "i");
        export_disc_snap(&state_dir, &source).expect("export");

        let host = read_export(&state_dir, "host.json");
        assert_eq!(host.kind, "disc_snap");
        assert_eq!(host.source.probe, Some("host".to_string()));
    }

    #[test]
    fn export_metrics_writes_envelope_with_correct_kind() {
        let state_dir = temp_dir("metrics");
        fs::create_dir_all(state_dir.join("telemetry")).expect("create telemetry dir");

        use warp_insight_contracts::discovery::StringKeyValue;
        use crate::telemetry::metrics::runtime::{MetricsCollectionOutcome, MetricsCollectionTargetSample};

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

        let output = read_export(&state_dir, "metrics.json");
        assert_eq!(output.api_version, "warp-insight/v1");
        assert_eq!(output.kind, "metrics");
        assert_eq!(output.payload["groups"][0]["samples"][0]["name"], "system.uptime");
        assert_eq!(output.payload["groups"][0]["samples"][0]["value"], 42.0);
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
        write_cache(&state_dir,
            serde_json::json!([{"resource_id":"h1","kind":"host","attributes":{},"discovered_at":"","last_seen_at":"","health":"healthy","source":"host"}]),
            serde_json::json!([{"target_id":"h1:host","kind":"host","resource_ref":"h1","execution_hints":{},"state":"active"}]),
        );
        let source = ExporterSource::new("a", "i");

        export_disc_snap(&state_dir, &source).expect("first");
        let first = read_export(&state_dir, "host.json");
        export_disc_snap(&state_dir, &source).expect("second");
        let second = read_export(&state_dir, "host.json");

        assert!(second.seq > first.seq, "seq must increase");
        assert!(first.seq >= BASE);
    }

    #[test]
    fn strips_origin_idx_from_export_not_cache() {
        let state_dir = temp_dir("strip");
        write_cache(&state_dir,
            serde_json::json!([{"resource_id":"h1","kind":"host","origin_idx":5,"attributes":{},"discovered_at":"","last_seen_at":"","health":"healthy","source":"host"}]),
            serde_json::json!([{"target_id":"h1:host","kind":"host","origin_idx":5,"resource_ref":"h1","execution_hints":{},"state":"active"}]),
        );
        let source = ExporterSource::new("a", "b");
        export_disc_snap(&state_dir, &source).expect("export");

        let cached: serde_json::Value = read_json(&discovery_cache::DiscoveryCachePaths::under_state_dir(&state_dir).resources).expect("cache");
        assert!(cached[0].get("origin_idx").is_some(), "cache keeps origin_idx");

        let output = read_export(&state_dir, "host.json");
        assert!(output.payload["resources"][0].get("origin_idx").is_none(), "export strips origin_idx");
    }

    #[test]
    fn per_probe_files_contain_only_matching_kind() {
        let state_dir = temp_dir("per-probe");
        let paths = discovery_cache::DiscoveryCachePaths::under_state_dir(&state_dir);
        fs::create_dir_all(&paths.root).expect("create dir");

        let resources = serde_json::json!([
            {"resource_id":"h1","kind":"host","attributes":{},"discovered_at":"","last_seen_at":"","health":"healthy","source":"host"},
            {"resource_id":"p1","kind":"process","attributes":{},"discovered_at":"","last_seen_at":"","health":"healthy","source":"process"},
        ]);
        let meta = serde_json::json!({"schema_version": "v1", "snapshot_id": "s", "revision": 1, "generated_at": "", "origins": []});
        fs::write(&paths.resources, resources.to_string()).expect("write");
        fs::write(&paths.meta, meta.to_string()).expect("write");
        fs::write(&paths.targets, "[]").expect("write");

        export_disc_snap(&state_dir, &ExporterSource::new("a", "b")).expect("export");

        let host: ExporterOutput<serde_json::Value> = read_json(&state_dir.join("export").join("host.json")).expect("host");
        assert_eq!(host.payload["resources"].as_array().unwrap().len(), 1);
        assert_eq!(host.payload["resources"][0]["resource_id"], "h1");
        assert_eq!(host.source.probe.as_deref(), Some("host"));

        let proc_path = state_dir.join("export").join("process-unidentified.jsonl");
        assert!(proc_path.exists());
        let lines = count_jsonl(&proc_path).expect("count");
        assert!(lines >= 2);
    }
}
