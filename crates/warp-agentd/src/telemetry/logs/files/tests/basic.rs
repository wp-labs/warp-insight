use std::fs;

use super::{
    FileInputProcessor, ReadLimits, TestSink, config, log_checkpoints, read_json,
    read_output_records, temp_dir,
};
use crate::telemetry::logs::files::file_watcher::StartupPosition;
use crate::telemetry::warp_parse::FileRecordSink;

#[test]
fn processes_new_lines_and_advances_checkpoint_after_successful_commit() {
    let root = temp_dir("success");
    let source_path = root.join("app.log");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::write(&source_path, "first\nsecond\n").expect("write log");
    let mut processor = FileInputProcessor::new(config(&root, &source_path), TestSink::default());

    let outcome = processor.process_once().expect("process");
    let checkpoint_path = log_checkpoints::path_for(&root.join("state"), "input-app");
    let state: crate::state_store::log_checkpoint_state::LogCheckpointState =
        read_json(&checkpoint_path).expect("read checkpoint");

    assert_eq!(outcome.records_processed, 2);
    assert_eq!(outcome.emitted_directly, 2);
    assert_eq!(outcome.spooled, 0);
    assert_eq!(
        state.files[0].checkpoint_offset,
        "first\nsecond\n".len() as u64
    );
}

#[test]
fn does_not_advance_to_partial_line_tail() {
    let root = temp_dir("partial");
    let source_path = root.join("app.log");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::write(&source_path, "first\nsecond").expect("write log");
    let mut processor = FileInputProcessor::new(config(&root, &source_path), TestSink::default());

    let outcome = processor.process_once().expect("process");

    assert_eq!(outcome.records_processed, 1);
    assert_eq!(outcome.checkpoint_offset, "first\n".len() as u64);
}

#[test]
fn restart_recovery_reads_only_appended_lines_after_checkpoint() {
    let root = temp_dir("restart");
    let source_path = root.join("app.log");
    let first_line = "abcdefghijklmnopqrstuvwxyz123456\n";
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::write(&source_path, first_line).expect("write log");
    let mut first = FileInputProcessor::new(config(&root, &source_path), TestSink::default());
    first.process_once().expect("first process");

    fs::write(&source_path, format!("{first_line}second\n")).expect("append log");
    let mut second = FileInputProcessor::new(config(&root, &source_path), TestSink::default());
    let outcome = second.process_once().expect("second process");

    assert_eq!(outcome.records_processed, 1);
    assert_eq!(
        outcome.checkpoint_offset,
        (first_line.len() + "second\n".len()) as u64
    );
}

#[test]
fn short_file_append_keeps_checkpoint_and_reads_only_new_line() {
    let root = temp_dir("short-restart");
    let source_path = root.join("app.log");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::write(&source_path, "abc\n").expect("write log");
    let mut first = FileInputProcessor::new(config(&root, &source_path), TestSink::default());
    first.process_once().expect("first process");

    fs::write(&source_path, "abc\ndef\n").expect("append log");
    let mut second = FileInputProcessor::new(config(&root, &source_path), TestSink::default());
    let outcome = second.process_once().expect("second process");

    assert_eq!(outcome.records_processed, 1);
    assert_eq!(outcome.checkpoint_offset, "abc\ndef\n".len() as u64);
}

#[test]
fn detects_truncate_and_restarts_from_zero() {
    let root = temp_dir("truncate");
    let source_path = root.join("app.log");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::write(&source_path, "first\nsecond\n").expect("write log");
    let mut first = FileInputProcessor::new(config(&root, &source_path), TestSink::default());
    first.process_once().expect("first process");

    fs::write(&source_path, "short\n").expect("truncate log");
    let mut second = FileInputProcessor::new(config(&root, &source_path), TestSink::default());
    let outcome = second.process_once().expect("second process");

    assert!(outcome.truncated || outcome.rotated);
    assert_eq!(outcome.records_processed, 1);
    assert_eq!(outcome.checkpoint_offset, "short\n".len() as u64);
}

#[test]
fn startup_position_tail_skips_existing_content_until_new_append() {
    let root = temp_dir("startup-tail");
    let source_path = root.join("app.log");
    let output_path = root.join("log").join("records.ndjson");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::create_dir_all(root.join("log")).expect("create log");
    fs::write(&source_path, "existing\n").expect("write log");
    let mut cfg = config(&root, &source_path);
    cfg.startup_position = StartupPosition::Tail;

    let mut first = FileInputProcessor::new(cfg.clone(), FileRecordSink::new(output_path.clone()));
    let first_outcome = first.process_once().expect("first process");

    assert_eq!(first_outcome.records_processed, 0);
    assert_eq!(first_outcome.checkpoint_offset, "existing\n".len() as u64);
    assert!(!output_path.exists());

    fs::write(&source_path, "existing\nnew\n").expect("append log");
    let mut second = FileInputProcessor::new(cfg, FileRecordSink::new(output_path.clone()));
    let second_outcome = second.process_once().expect("second process");
    let records = read_output_records(&output_path);

    assert_eq!(second_outcome.records_processed, 1);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].body, "new\n");
}

#[test]
fn over_long_line_is_truncated_committed_and_counted() {
    let root = temp_dir("truncate-long-line");
    let source_path = root.join("app.log");
    fs::create_dir_all(root.join("state")).expect("create state");
    let long_line = "x".repeat(2048);
    let content = format!("{long_line}\nnext\n");
    fs::write(&source_path, &content).expect("write log");
    let mut cfg = config(&root, &source_path);
    cfg.read_limits = ReadLimits::new(1024, 1_048_576, 64);
    let mut processor = FileInputProcessor::new(cfg, TestSink::default());

    let outcome = processor.process_once().expect("process");

    assert_eq!(outcome.truncated_lines, 1);
    assert_eq!(outcome.records_processed, 2);
    assert_eq!(outcome.checkpoint_offset, content.len() as u64);
}

#[test]
fn chunked_read_by_max_lines_advances_checkpoint_each_tick_without_loss() {
    let root = temp_dir("chunked-process");
    let source_path = root.join("app.log");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::write(&source_path, "one\ntwo\nthree\n").expect("write log");
    let mut cfg = config(&root, &source_path);
    cfg.read_limits = ReadLimits::new(1_048_576, 1_048_576, 1);

    let mut first = FileInputProcessor::new(cfg.clone(), TestSink::default());
    let a = first.process_once().expect("first");
    assert_eq!(a.records_processed, 1);
    assert_eq!(a.checkpoint_offset, "one\n".len() as u64);

    let mut second = FileInputProcessor::new(cfg.clone(), TestSink::default());
    let b = second.process_once().expect("second");
    assert_eq!(b.records_processed, 1);
    assert_eq!(b.checkpoint_offset, "one\ntwo\n".len() as u64);

    let mut third = FileInputProcessor::new(cfg, TestSink::default());
    let c = third.process_once().expect("third");
    assert_eq!(c.records_processed, 1);
    assert_eq!(c.checkpoint_offset, "one\ntwo\nthree\n".len() as u64);
}

#[test]
fn line_over_read_budget_is_delivered_without_loss() {
    let root = temp_dir("over-budget-line");
    let source_path = root.join("app.log");
    fs::create_dir_all(root.join("state")).expect("create state");
    let long_line = "z".repeat(20_000);
    let content = format!("{long_line}\nnext\n");
    fs::write(&source_path, &content).expect("write log");
    let mut cfg = config(&root, &source_path);
    // 预算 16 字节 << 行长；单行上限 64KiB 不截断。
    cfg.read_limits = ReadLimits::new(65_536, 16, 64);

    let mut processor = FileInputProcessor::new(cfg, TestSink::default());
    let outcome = processor.process_once().expect("process");

    assert_eq!(outcome.truncated_lines, 0);
    assert_eq!(outcome.records_processed, 1);
    assert_eq!(outcome.checkpoint_offset, (long_line.len() + 1) as u64);
}

#[test]
fn assigns_monotonic_seq_and_persists_next_seq() {
    let root = temp_dir("seq-persist");
    let source_path = root.join("app.log");
    let output_path = root.join("log").join("records.ndjson");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::create_dir_all(root.join("log")).expect("create log");
    fs::write(&source_path, "first\nsecond\n").expect("write log");

    let mut processor = FileInputProcessor::new(
        config(&root, &source_path),
        FileRecordSink::new(output_path.clone()),
    );
    let outcome = processor.process_once().expect("process");

    assert_eq!(outcome.records_processed, 2);
    let records = read_output_records(&output_path);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].seq, 0);
    assert_eq!(records[1].seq, 1);

    let checkpoint_path = log_checkpoints::path_for(&root.join("state"), "input-app");
    let state: crate::state_store::log_checkpoint_state::LogCheckpointState =
        read_json(&checkpoint_path).expect("read checkpoint");
    assert_eq!(state.next_seq, 2);
}

#[test]
fn restart_resumes_seq_from_persisted_next_seq() {
    let root = temp_dir("seq-restart");
    let source_path = root.join("app.log");
    let output_path = root.join("log").join("records.ndjson");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::create_dir_all(root.join("log")).expect("create log");
    fs::write(&source_path, "first\nsecond\n").expect("write log");

    let mut first = FileInputProcessor::new(
        config(&root, &source_path),
        FileRecordSink::new(output_path.clone()),
    );
    first.process_once().expect("first process");

    // 重启：追加新行，seq 应从持久化的 next_seq=2 续号。
    fs::write(&source_path, "first\nsecond\nthird\n").expect("append log");
    let mut second = FileInputProcessor::new(
        config(&root, &source_path),
        FileRecordSink::new(output_path.clone()),
    );
    second.process_once().expect("second process");

    let records = read_output_records(&output_path);
    assert_eq!(records.len(), 3);
    assert_eq!(records[2].seq, 2);
}
