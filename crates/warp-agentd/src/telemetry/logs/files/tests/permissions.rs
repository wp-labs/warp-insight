use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;

use super::{FileInputProcessor, TestSink, config, log_checkpoints, temp_dir};

#[test]
fn unreadable_source_does_not_advance_checkpoint_and_recovers_after_fix() {
    let root = temp_dir("permission");
    let source_path = root.join("app.log");
    fs::create_dir_all(root.join("state")).expect("create state");
    fs::write(&source_path, "first\nsecond\n").expect("write log");
    fs::set_permissions(&source_path, fs::Permissions::from_mode(0o000)).expect("chmod 000");

    if fs::read(&source_path).is_ok() {
        // 以 root 等特权运行时 0o000 仍可读，跳过该用例。
        fs::set_permissions(&source_path, fs::Permissions::from_mode(0o644)).ok();
        return;
    }

    let mut blocked = FileInputProcessor::new(config(&root, &source_path), TestSink::default());
    let err = blocked
        .process_once()
        .expect_err("unreadable source should fail");
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);

    let checkpoint_path = log_checkpoints::path_for(&root.join("state"), "input-app");
    assert!(
        !checkpoint_path.exists(),
        "checkpoint must not advance on read failure"
    );

    fs::set_permissions(&source_path, fs::Permissions::from_mode(0o644)).expect("chmod 644");
    let mut resumed = FileInputProcessor::new(config(&root, &source_path), TestSink::default());
    let outcome = resumed
        .process_once()
        .expect("process after permission fix");

    assert_eq!(outcome.records_processed, 2);
    assert_eq!(outcome.checkpoint_offset, "first\nsecond\n".len() as u64);
}
