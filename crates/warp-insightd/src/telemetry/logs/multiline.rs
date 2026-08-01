//! Minimal multiline folding for log records.

use crate::state_store::log_checkpoint_state::PendingMultilineState;

use super::file_reader::RawFileLine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultilineMode {
    None,
    IndentedContinuation,
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct FoldedLine {
    pub body: String,
    pub start_offset: u64,
    pub end_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct FoldResult {
    pub emitted: Vec<FoldedLine>,
    pub pending: Option<PendingMultilineState>,
}

pub fn flush_pending(pending: Option<PendingMultilineState>) -> Vec<FoldedLine> {
    pending.into_iter().map(folded_from_pending).collect()
}

pub fn fold_lines(
    mode: MultilineMode,
    source_path: &str,
    observed_at: &str,
    lines: Vec<RawFileLine>,
    pending: Option<PendingMultilineState>,
) -> FoldResult {
    match mode {
        MultilineMode::None => FoldResult {
            emitted: flush_pending(pending)
                .into_iter()
                .chain(lines.into_iter().map(|line| FoldedLine {
                    body: line.text,
                    start_offset: line.start_offset,
                    end_offset: line.end_offset,
                }))
                .collect(),
            pending: None,
        },
        MultilineMode::IndentedContinuation => {
            fold_indented(source_path, observed_at, lines, pending)
        }
    }
}

fn fold_indented(
    source_path: &str,
    observed_at: &str,
    lines: Vec<RawFileLine>,
    pending: Option<PendingMultilineState>,
) -> FoldResult {
    let mut emitted = Vec::new();
    let mut current = pending.filter(|entry| entry.source_path == source_path);
    for line in lines {
        if starts_with_indent(&line.text) {
            if let Some(existing) = current.as_mut() {
                existing.body.push_str(&line.text);
                existing.end_offset = line.end_offset;
                existing.last_updated_at = observed_at.to_string();
                continue;
            }
        }

        if let Some(previous) = current.take() {
            emitted.push(folded_from_pending(previous));
        }
        current = Some(PendingMultilineState {
            source_path: source_path.to_string(),
            body: line.text,
            start_offset: line.start_offset,
            end_offset: line.end_offset,
            last_updated_at: observed_at.to_string(),
        });
    }

    FoldResult {
        emitted,
        pending: current,
    }
}

fn folded_from_pending(pending: PendingMultilineState) -> FoldedLine {
    FoldedLine {
        body: pending.body,
        start_offset: pending.start_offset,
        end_offset: pending.end_offset,
    }
}

fn starts_with_indent(text: &str) -> bool {
    matches!(text.as_bytes().first(), Some(b' ' | b'\t'))
}

#[cfg(test)]
mod tests {
    use super::{MultilineMode, flush_pending, fold_lines};
    use crate::state_store::log_checkpoint_state::PendingMultilineState;
    use crate::telemetry::logs::file_reader::RawFileLine;

    #[test]
    fn indented_mode_keeps_last_group_pending_until_next_tick() {
        let first = fold_lines(
            MultilineMode::IndentedContinuation,
            "/tmp/app.log",
            "2026-04-14T00:00:00Z",
            vec![RawFileLine {
                text: "ERROR first line\n".to_string(),
                start_offset: 0,
                end_offset: 17,
            }],
            None,
        );

        assert!(first.emitted.is_empty());
        assert_eq!(
            first.pending,
            Some(PendingMultilineState {
                source_path: "/tmp/app.log".to_string(),
                body: "ERROR first line\n".to_string(),
                start_offset: 0,
                end_offset: 17,
                last_updated_at: "2026-04-14T00:00:00Z".to_string(),
            })
        );

        let second = fold_lines(
            MultilineMode::IndentedContinuation,
            "/tmp/app.log",
            "2026-04-14T00:00:01Z",
            vec![
                RawFileLine {
                    text: "  stack frame 1\n".to_string(),
                    start_offset: 17,
                    end_offset: 33,
                },
                RawFileLine {
                    text: "INFO next\n".to_string(),
                    start_offset: 33,
                    end_offset: 43,
                },
            ],
            first.pending,
        );

        assert_eq!(second.emitted.len(), 1);
        assert_eq!(
            second.emitted[0].body,
            "ERROR first line\n  stack frame 1\n"
        );
        assert_eq!(second.emitted[0].start_offset, 0);
        assert_eq!(second.emitted[0].end_offset, 33);
        assert_eq!(
            second.pending,
            Some(PendingMultilineState {
                source_path: "/tmp/app.log".to_string(),
                body: "INFO next\n".to_string(),
                start_offset: 33,
                end_offset: 43,
                last_updated_at: "2026-04-14T00:00:01Z".to_string(),
            })
        );
    }

    #[test]
    fn flush_pending_turns_state_back_into_record() {
        let flushed = flush_pending(Some(PendingMultilineState {
            source_path: "/tmp/app.log".to_string(),
            body: "INFO next\n".to_string(),
            start_offset: 33,
            end_offset: 43,
            last_updated_at: "2026-04-14T00:00:01Z".to_string(),
        }));

        assert_eq!(flushed.len(), 1);
        assert_eq!(flushed[0].body, "INFO next\n");
        assert_eq!(flushed[0].start_offset, 33);
        assert_eq!(flushed[0].end_offset, 43);
    }

    #[test]
    fn none_mode_emits_pending_and_current_lines_immediately() {
        let folded = fold_lines(
            MultilineMode::None,
            "/tmp/app.log",
            "2026-04-14T00:00:00Z",
            vec![RawFileLine {
                text: "second\n".to_string(),
                start_offset: 6,
                end_offset: 13,
            }],
            Some(PendingMultilineState {
                source_path: "/tmp/app.log".to_string(),
                body: "first\n".to_string(),
                start_offset: 0,
                end_offset: 6,
                last_updated_at: "2026-04-14T00:00:00Z".to_string(),
            }),
        );

        assert_eq!(folded.emitted.len(), 2);
        assert!(folded.pending.is_none());
    }
}
