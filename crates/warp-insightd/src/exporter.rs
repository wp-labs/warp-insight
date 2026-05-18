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
const DISCOVERY_PROBES: &[&str] = &[
    "host",
    "network_interface",
    "ip_address",
    "service_endpoint",
    "process",
    "container",
];

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
            target.get("resource_ref").and_then(|value| value.as_str()) == Some(resource_id)
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

struct DiscRowContext<'a> {
    source: &'a ExporterSource,
    probe: &'a str,
    classification: Option<&'a str>,
    output_id: &'a str,
    seq: u64,
    generated_at: &'a str,
    snapshot_id: &'a str,
    snapshot_revision: u64,
    snapshot_generated_at: &'a str,
}

fn build_disc_row(
    context: &DiscRowContext<'_>,
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
        serde_json::Value::String(context.output_id.to_string()),
    );
    row.insert(
        "seq".to_string(),
        serde_json::Value::Number(context.seq.into()),
    );
    row.insert(
        "generated_at".to_string(),
        serde_json::Value::String(context.generated_at.to_string()),
    );
    row.insert(
        "source".to_string(),
        serde_json::to_value(context.source.clone().with_probe(context.probe))
            .expect("serialize source"),
    );
    row.insert(
        "snapshot_id".to_string(),
        serde_json::Value::String(context.snapshot_id.to_string()),
    );
    row.insert(
        "snapshot_revision".to_string(),
        serde_json::Value::Number(context.snapshot_revision.into()),
    );
    row.insert(
        "snapshot_generated_at".to_string(),
        serde_json::Value::String(context.snapshot_generated_at.to_string()),
    );
    if let Some(classification) = context.classification {
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
    let row_context = DiscRowContext {
        source,
        probe,
        classification: None,
        output_id: &output_id,
        seq,
        generated_at: &generated_at,
        snapshot_id: &snapshot_id,
        snapshot_revision,
        snapshot_generated_at: &snapshot_generated_at,
    };

    let rows: Vec<_> = probe_resources
        .into_iter()
        .map(|resource| {
            let resource_id = resource
                .get("resource_id")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            build_disc_row(
                &row_context,
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
        } else if let Err(err) = export_probe(
            state_dir,
            source,
            probe,
            &resources,
            &targets,
            meta.as_ref(),
        ) {
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
        let row_context = DiscRowContext {
            source,
            probe: "process",
            classification: Some(set_name),
            output_id: &output_id,
            seq,
            generated_at: &generated_at,
            snapshot_id: &snapshot_id,
            snapshot_revision,
            snapshot_generated_at: &snapshot_generated_at,
        };
        let rows: Vec<_> = proc_resources
            .into_iter()
            .map(|resource| {
                let resource_id = resource
                    .get("resource_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                build_disc_row(
                    &row_context,
                    resource,
                    matching_target(&process_targets, &resource_id),
                )
            })
            .collect();

        let out_path = state_dir
            .join("export")
            .join(format!("process-{set_name}.jsonl"));
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
#[path = "exporter_tests.rs"]
mod tests;
