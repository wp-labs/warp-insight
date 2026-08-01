//! Resume and rollover decisions for tracked files.

use std::io;
use std::path::Path;

use crate::state_store::log_checkpoint_state::TrackedFileCheckpoint;

use super::file_reader::{ObservedFileIdentity, checkpoint_probe};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupPosition {
    Head,
    Tail,
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct ResumeDecision {
    pub start_offset: u64,
    pub rotated_from_path: Option<String>,
    pub truncated: bool,
    pub rotated: bool,
}

pub fn decide_resume(
    source_path: &Path,
    current: &ObservedFileIdentity,
    previous: Option<&TrackedFileCheckpoint>,
    startup_position: StartupPosition,
) -> ResumeDecision {
    let source_path = source_path.display().to_string();
    let Some(previous) = previous else {
        return ResumeDecision {
            start_offset: match startup_position {
                StartupPosition::Head => 0,
                StartupPosition::Tail => current.file_len,
            },
            rotated_from_path: None,
            truncated: false,
            rotated: false,
        };
    };

    let same_path = previous.path.is_empty() || previous.path == source_path;
    let same_identity = if previous.device_id.is_some()
        && previous.inode.is_some()
        && current.device_id.is_some()
        && current.inode.is_some()
    {
        same_path
            && previous.device_id == current.device_id
            && previous.inode == current.inode
            && fingerprints_match(previous, current)
    } else {
        same_path && fingerprints_match(previous, current)
    };

    if same_identity {
        if previous.checkpoint_offset > current.file_len
            || !checkpoint_probe_matches(Path::new(&source_path), previous).unwrap_or(false)
        {
            return ResumeDecision {
                start_offset: 0,
                rotated_from_path: None,
                truncated: true,
                rotated: false,
            };
        }
        return ResumeDecision {
            start_offset: previous.checkpoint_offset,
            rotated_from_path: None,
            truncated: false,
            rotated: false,
        };
    }

    ResumeDecision {
        start_offset: 0,
        rotated_from_path: Some(previous.path.clone().if_empty_then(source_path.to_string())),
        truncated: false,
        rotated: true,
    }
}

trait IfEmptyThen {
    fn if_empty_then(self, fallback: String) -> String;
}

impl IfEmptyThen for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.is_empty() { fallback } else { self }
    }
}

fn fingerprints_match(previous: &TrackedFileCheckpoint, current: &ObservedFileIdentity) -> bool {
    match (
        previous.fingerprint.as_deref(),
        current.fingerprint.as_deref(),
    ) {
        (Some(previous_fingerprint), Some(current_fingerprint)) => {
            previous_fingerprint == current_fingerprint
                || short_file_append_keeps_prefix(
                    previous,
                    current,
                    previous_fingerprint,
                    current_fingerprint,
                )
        }
        _ => true,
    }
}

fn short_file_append_keeps_prefix(
    previous: &TrackedFileCheckpoint,
    current: &ObservedFileIdentity,
    previous_fingerprint: &str,
    current_fingerprint: &str,
) -> bool {
    let Some(previous_size) = previous.last_size else {
        return false;
    };
    previous_size < 32
        && current.file_len > previous_size
        && current_fingerprint.starts_with(previous_fingerprint)
}

fn checkpoint_probe_matches(path: &Path, previous: &TrackedFileCheckpoint) -> io::Result<bool> {
    if previous.checkpoint_offset == 0 {
        return Ok(true);
    }
    let Some(expected) = previous.checkpoint_probe.as_deref() else {
        return Ok(true);
    };
    Ok(checkpoint_probe(path, previous.checkpoint_offset)?.as_deref() == Some(expected))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ResumeDecision, StartupPosition, decide_resume};
    use crate::state_store::log_checkpoint_state::TrackedFileCheckpoint;
    use crate::telemetry::logs::file_reader::ObservedFileIdentity;

    fn temp_file(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        std::env::temp_dir().join(format!("warp-insightd-file-watcher-{name}-{suffix}.log"))
    }

    fn previous(offset: u64) -> TrackedFileCheckpoint {
        TrackedFileCheckpoint {
            file_id: "dev:1:ino:2".to_string(),
            path: "/tmp/app.log".to_string(),
            device_id: Some(1),
            inode: Some(2),
            fingerprint: Some("616263".to_string()),
            checkpoint_offset: offset,
            checkpoint_probe: None,
            last_size: Some(offset),
            last_read_at: None,
            last_commit_point_at: None,
            rotated_from_path: None,
        }
    }

    #[test]
    fn reuses_checkpoint_when_identity_is_unchanged() {
        let current = ObservedFileIdentity {
            device_id: Some(1),
            inode: Some(2),
            fingerprint: Some("616263".to_string()),
            file_len: 10,
        };

        let decision = decide_resume(
            Path::new("/tmp/app.log"),
            &current,
            Some(&previous(4)),
            StartupPosition::Head,
        );

        assert_eq!(
            decision,
            ResumeDecision {
                start_offset: 4,
                rotated_from_path: None,
                truncated: false,
                rotated: false,
            }
        );
    }

    #[test]
    fn resets_to_zero_when_checkpoint_exceeds_current_file_len() {
        let current = ObservedFileIdentity {
            device_id: Some(1),
            inode: Some(2),
            fingerprint: Some("616263".to_string()),
            file_len: 2,
        };

        let decision = decide_resume(
            Path::new("/tmp/app.log"),
            &current,
            Some(&previous(4)),
            StartupPosition::Head,
        );

        assert!(decision.truncated);
        assert_eq!(decision.start_offset, 0);
    }

    #[test]
    fn treats_same_inode_with_changed_fingerprint_as_rotation() {
        let current = ObservedFileIdentity {
            device_id: Some(1),
            inode: Some(2),
            fingerprint: Some("646566".to_string()),
            file_len: 10,
        };

        let decision = decide_resume(
            Path::new("/tmp/app.log"),
            &current,
            Some(&previous(4)),
            StartupPosition::Head,
        );

        assert!(decision.rotated);
        assert_eq!(decision.start_offset, 0);
    }

    #[test]
    fn allows_short_file_to_grow_without_resetting_checkpoint() {
        let current = ObservedFileIdentity {
            device_id: Some(1),
            inode: Some(2),
            fingerprint: Some("616263646566".to_string()),
            file_len: 6,
        };

        let decision = decide_resume(
            Path::new("/tmp/app.log"),
            &current,
            Some(&previous(3)),
            StartupPosition::Head,
        );

        assert!(!decision.rotated);
        assert_eq!(decision.start_offset, 3);
    }

    #[test]
    fn starts_from_end_when_no_checkpoint_and_tail_is_requested() {
        let current = ObservedFileIdentity {
            device_id: Some(1),
            inode: Some(2),
            fingerprint: Some("616263".to_string()),
            file_len: 10,
        };

        let decision = decide_resume(
            Path::new("/tmp/app.log"),
            &current,
            None,
            StartupPosition::Tail,
        );

        assert_eq!(
            decision,
            ResumeDecision {
                start_offset: 10,
                rotated_from_path: None,
                truncated: false,
                rotated: false,
            }
        );
    }

    #[test]
    fn detects_copytruncate_when_probe_at_checkpoint_changes() {
        let path = temp_file("copytruncate");
        fs::write(&path, "1234567890abcdef\nold-tail\n").expect("write old file");
        let current = ObservedFileIdentity {
            device_id: Some(1),
            inode: Some(2),
            fingerprint: Some("313233343536373839306162636465660a6f6c642d7461696c0a".to_string()),
            file_len: "1234567890abcdef\nnew-tail\nmore\n".len() as u64,
        };
        let previous = TrackedFileCheckpoint {
            file_id: "dev:1:ino:2".to_string(),
            path: path.display().to_string(),
            device_id: Some(1),
            inode: Some(2),
            fingerprint: Some("313233343536373839306162636465660a6f6c642d7461696c0a".to_string()),
            checkpoint_offset: "1234567890abcdef\nold-tail\n".len() as u64,
            checkpoint_probe: Some("66660a6f6c642d7461696c0a".to_string()),
            last_size: Some("1234567890abcdef\nold-tail\n".len() as u64),
            last_read_at: None,
            last_commit_point_at: None,
            rotated_from_path: None,
        };
        fs::write(&path, "1234567890abcdef\nnew-tail\nmore\n").expect("rewrite file");

        let decision = decide_resume(&path, &current, Some(&previous), StartupPosition::Head);

        assert!(decision.truncated);
        assert_eq!(decision.start_offset, 0);
        fs::remove_file(path).ok();
    }
}
