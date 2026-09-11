use std::fs;
use std::io;

use super::{
    FileInputProcessor, TestSink, config, log_checkpoints, read_json, read_output_records, spool,
    temp_dir,
};
use crate::telemetry::warp_parse::FileRecordSink;

#[test]
fn sink_failure_spools_records_and_only_then_advances_checkpoint() {
    let root = temp_dir("spool");
    let source_path = root.join("app.log");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::write(&source_path, "first\nsecond\n").expect("write log");
    let mut processor = FileInputProcessor::new(
        config(&root, &source_path),
        TestSink {
            fail_writes: true,
            ..Default::default()
        },
    );

    let outcome = processor.process_once().expect("process");
    let spooled = spool::load_records(&root.join("state").join("spool").join("input-app.ndjson"))
        .expect("load spool");

    assert_eq!(outcome.emitted_directly, 0);
    assert_eq!(outcome.spooled, 2);
    assert_eq!(spooled.len(), 2);
    assert_eq!(outcome.checkpoint_offset, "first\nsecond\n".len() as u64);
}

#[test]
fn replays_spooled_records_after_sink_recovers_and_clears_spool() {
    let root = temp_dir("spool-replay");
    let source_path = root.join("app.log");
    let output_path = root.join("log").join("records.ndjson");
    let first_line = "abcdefghijklmnopqrstuvwxyz123456\n";
    let second_line = "ABCDEFGHIJKLMNOPQRSTUVWXYZ654321\n";
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::create_dir_all(root.join("log")).expect("create log");
    fs::write(&source_path, format!("{first_line}{second_line}")).expect("write log");

    let mut failing = FileInputProcessor::new(
        config(&root, &source_path),
        TestSink {
            fail_writes: true,
            ..Default::default()
        },
    );
    let first = failing.process_once().expect("first process");
    assert_eq!(first.spooled, 2);

    fs::write(&source_path, format!("{first_line}{second_line}third\n")).expect("append third");
    let mut replay = FileInputProcessor::new(
        config(&root, &source_path),
        FileRecordSink::new(output_path.clone()),
    );
    let second = replay.process_once().expect("second process");
    let spooled = spool::load_records(&root.join("state").join("spool").join("input-app.ndjson"))
        .expect("load spool");
    let records = read_output_records(&output_path);

    assert_eq!(second.replayed_spool, 2);
    assert_eq!(second.records_processed, 1);
    assert!(spooled.is_empty());
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].body, first_line);
    assert_eq!(records[1].body, second_line);
    assert_eq!(records[2].body, "third\n");
}

#[test]
fn replay_failure_is_reported_when_spool_exists_but_sink_is_still_unavailable() {
    let root = temp_dir("spool-replay-failure");
    let source_path = root.join("app.log");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::write(&source_path, "first\nsecond\n").expect("write log");

    let mut first = FileInputProcessor::new(
        config(&root, &source_path),
        TestSink {
            fail_writes: true,
            ..Default::default()
        },
    );
    first.process_once().expect("first process");

    let mut replay = FileInputProcessor::new(
        config(&root, &source_path),
        TestSink {
            fail_writes: true,
            ..Default::default()
        },
    );
    let err = replay.process_once().expect_err("replay should fail");

    assert_eq!(err.kind(), io::ErrorKind::Other);
    assert!(
        root.join("state")
            .join("spool")
            .join("input-app.ndjson")
            .exists()
    );
}

#[test]
fn spool_over_limit_pauses_source_read_and_resumes_after_drain() {
    let root = temp_dir("spool-limit");
    let source_path = root.join("app.log");
    let output_path = root.join("log").join("records.ndjson");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::create_dir_all(root.join("log")).expect("create log");
    fs::write(&source_path, "first\nsecond\n").expect("write log");

    let mut cfg = config(&root, &source_path);
    // 任意非空 spool 都视为超限，便于固定背压路径。
    cfg.spool_max_bytes = 1;
    let checkpoint_path = log_checkpoints::path_for(&root.join("state"), "input-app");

    // tick 1：sink 不可用 → 记录落入 spool，checkpoint 推进。
    let mut spooling = FileInputProcessor::new(
        cfg.clone(),
        TestSink {
            fail_writes: true,
            ..Default::default()
        },
    );
    let first = spooling.process_once().expect("first process");
    assert!(!first.paused);
    assert_eq!(first.spooled, 2);
    let checkpoint_after_first: crate::state_store::log_checkpoint_state::LogCheckpointState =
        read_json(&checkpoint_path).expect("read checkpoint");

    // tick 2：回放仍失败且 spool 超限 → 暂停，不读源、不推进 checkpoint。
    let mut paused = FileInputProcessor::new(
        cfg.clone(),
        TestSink {
            fail_writes: true,
            ..Default::default()
        },
    );
    let second = paused.process_once().expect("second process");
    assert!(second.paused);
    assert_eq!(
        second.kind,
        crate::telemetry::logs::files::file::ProcessOutcomeKind::SpoolPaused
    );
    assert!(second.spool_bytes > 0);
    let checkpoint_after_second: crate::state_store::log_checkpoint_state::LogCheckpointState =
        read_json(&checkpoint_path).expect("read checkpoint");
    assert_eq!(
        checkpoint_after_second.files[0].checkpoint_offset,
        checkpoint_after_first.files[0].checkpoint_offset
    );

    // tick 3：sink 恢复 → 回放清空 spool，采集自动恢复。
    let mut resumed = FileInputProcessor::new(cfg, FileRecordSink::new(output_path.clone()));
    let third = resumed.process_once().expect("third process");
    assert!(!third.paused);
    assert_eq!(third.replayed_spool, 2);
    assert!(
        !spool::has_records(&root.join("state").join("spool").join("input-app.ndjson"))
            .expect("spool presence")
    );
    assert_eq!(read_output_records(&output_path).len(), 2);
}

#[test]
fn spool_exactly_at_limit_pauses() {
    let root = temp_dir("spool-at-limit");
    let source_path = root.join("app.log");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::write(&source_path, "first\n").expect("write log");
    let spool_path = root.join("state").join("spool").join("input-app.ndjson");

    let mut cfg = config(&root, &source_path);
    cfg.spool_max_bytes = 1;
    let mut spooling = FileInputProcessor::new(
        cfg.clone(),
        TestSink {
            fail_writes: true,
            ..Default::default()
        },
    );
    spooling.process_once().expect("spool first");

    let spool_bytes = fs::metadata(&spool_path).expect("spool meta").len();
    cfg.spool_max_bytes = spool_bytes; // 恰好等于上限：`>=` 应视为超限

    let mut paused = FileInputProcessor::new(
        cfg,
        TestSink {
            fail_writes: true,
            ..Default::default()
        },
    );
    let outcome = paused.process_once().expect("process");

    assert!(outcome.paused);
    assert_eq!(outcome.spool_bytes, spool_bytes);
}

#[test]
fn spool_over_limit_with_healthy_sink_replays_without_pausing() {
    let root = temp_dir("spool-over-limit-healthy");
    let source_path = root.join("app.log");
    let output_path = root.join("log").join("records.ndjson");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::create_dir_all(root.join("log")).expect("create log");
    fs::write(&source_path, "first\nsecond\n").expect("write log");
    let spool_path = root.join("state").join("spool").join("input-app.ndjson");

    let mut cfg = config(&root, &source_path);
    cfg.spool_max_bytes = 1;
    let mut spooling = FileInputProcessor::new(
        cfg.clone(),
        TestSink {
            fail_writes: true,
            ..Default::default()
        },
    );
    spooling.process_once().expect("spool first");
    assert!(spool::has_records(&spool_path).expect("spool presence"));

    // spool 已超限，但 sink 健康：应先回放清空，而不是暂停。
    let mut replay = FileInputProcessor::new(cfg, FileRecordSink::new(output_path.clone()));
    let outcome = replay.process_once().expect("replay");

    assert!(!outcome.paused);
    assert_eq!(outcome.replayed_spool, 2);
    assert!(!spool_path.exists());
    assert_eq!(read_output_records(&output_path).len(), 2);
}
