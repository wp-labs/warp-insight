use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use wist_contracts::agent_config::AgentConfigContract;
use wist_shared::paths::REPORT_ENVELOPE_SUFFIX;
use wist_shared::time::now_rfc3339;

use super::telemetry_support::{
    TelemetryFailure, TelemetryFailureKind, TelemetryWorkState, WorkState,
};

pub(super) fn emit_telemetry_failures(failures: &[TelemetryFailure]) {
    for failure in failures {
        emit_telemetry_failure(failure);
    }
}

pub(super) fn emit_telemetry_failure(failure: &TelemetryFailure) {
    match failure.kind {
        TelemetryFailureKind::MissingInput => eprintln!(
            "telemetry input missing input_id={} path={}",
            failure.input_id, failure.path
        ),
        TelemetryFailureKind::ProcessingFailed => eprintln!(
            "telemetry input failed input_id={} path={} err={}",
            failure.input_id, failure.path, failure.detail
        ),
        TelemetryFailureKind::InvalidOutput => eprintln!(
            "telemetry output invalid input_id={} path={} err={}",
            failure.input_id, failure.path, failure.detail
        ),
    }
}

pub(super) fn emit_work_state_notifications(notifications: &[TelemetryWorkState]) {
    for notification in notifications {
        emit_work_state_notification(notification);
    }
}

pub(super) fn emit_work_state_notification(notification: &TelemetryWorkState) {
    match notification.state {
        WorkState::Paused => eprintln!(
            "telemetry work-state paused input_id={} reason={} at={}",
            notification.input_id, notification.reason, notification.at
        ),
        WorkState::Resumed => eprintln!(
            "telemetry work-state resumed input_id={} reason={} at={}",
            notification.input_id, notification.reason, notification.at
        ),
    }
}

pub(super) fn paused_input_signatures(notifications: &[TelemetryWorkState]) -> BTreeSet<String> {
    notifications
        .iter()
        .map(|notification| notification.input_id.clone())
        .collect()
}

pub(super) fn work_state_changes(
    previous: &BTreeSet<String>,
    current_notifications: &[TelemetryWorkState],
    current: &BTreeSet<String>,
) -> Vec<TelemetryWorkState> {
    let mut changes = Vec::new();

    // 进入暂停：当前有、上次没有。
    for notification in current_notifications {
        if !previous.contains(&notification.input_id) {
            changes.push(notification.clone());
        }
    }

    // 恢复：上次有、当前没有。
    for input_id in previous.difference(current) {
        changes.push(TelemetryWorkState {
            input_id: input_id.clone(),
            state: WorkState::Resumed,
            reason: "spool replay recovered".to_string(),
            at: now_rfc3339(),
        });
    }

    changes
}

pub(super) fn failure_signatures(failures: &[TelemetryFailure]) -> BTreeSet<String> {
    failures.iter().map(failure_signature).collect()
}

pub(super) fn filter_new_failures<'a>(
    failures: &'a [TelemetryFailure],
    previous: &BTreeSet<String>,
) -> Vec<&'a TelemetryFailure> {
    failures
        .iter()
        .filter(|failure| !previous.contains(&failure_signature(failure)))
        .collect()
}

fn failure_signature(failure: &TelemetryFailure) -> String {
    format!(
        "{:?}|{}|{}|{}",
        failure.kind, failure.input_id, failure.path, failure.detail
    )
}

pub(super) async fn count_running_entries_async(state_dir: &Path) -> io::Result<usize> {
    let running_dir = state_dir.join("running");
    match tokio::fs::metadata(&running_dir).await {
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err),
    }

    let mut count = 0usize;
    let mut entries = tokio::fs::read_dir(running_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.path().extension().and_then(|ext| ext.to_str()) == Some("json") {
            count += 1;
        }
    }
    Ok(count)
}

pub(super) async fn count_reporting_entries_async(state_dir: &Path) -> io::Result<usize> {
    let reporting_dir = state_dir.join("reporting");
    match tokio::fs::metadata(&reporting_dir).await {
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err),
    }

    let mut count = 0usize;
    let mut entries = tokio::fs::read_dir(reporting_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.ends_with(REPORT_ENVELOPE_SUFFIX) {
            continue;
        }
        count += 1;
    }
    Ok(count)
}

pub(super) fn instance_id(config: &AgentConfigContract) -> String {
    config
        .agent
        .instance_name
        .clone()
        .unwrap_or_else(|| "local-instance".to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        TelemetryFailure, TelemetryFailureKind, TelemetryWorkState, WorkState, failure_signatures,
        filter_new_failures, paused_input_signatures, work_state_changes,
    };

    fn failure(
        kind: TelemetryFailureKind,
        input_id: &str,
        path: &str,
        detail: &str,
    ) -> TelemetryFailure {
        TelemetryFailure {
            kind,
            input_id: input_id.to_string(),
            path: path.to_string(),
            detail: detail.to_string(),
        }
    }

    fn work_state(input_id: &str, state: WorkState, reason: &str) -> TelemetryWorkState {
        TelemetryWorkState {
            input_id: input_id.to_string(),
            state,
            reason: reason.to_string(),
            at: "now".to_string(),
        }
    }

    #[test]
    fn failure_signatures_use_all_failure_identity_fields() {
        let signatures = failure_signatures(&[
            failure(
                TelemetryFailureKind::MissingInput,
                "app",
                "/tmp/a.log",
                "missing",
            ),
            failure(
                TelemetryFailureKind::MissingInput,
                "app",
                "/tmp/a.log",
                "missing",
            ),
            failure(
                TelemetryFailureKind::ProcessingFailed,
                "app",
                "/tmp/a.log",
                "failed",
            ),
        ]);

        assert_eq!(
            signatures,
            BTreeSet::from([
                "MissingInput|app|/tmp/a.log|missing".to_string(),
                "ProcessingFailed|app|/tmp/a.log|failed".to_string(),
            ])
        );
    }

    #[test]
    fn filter_new_failures_only_returns_entries_missing_from_previous_snapshot() {
        let failures = vec![
            failure(
                TelemetryFailureKind::MissingInput,
                "app",
                "/tmp/a.log",
                "missing",
            ),
            failure(
                TelemetryFailureKind::ProcessingFailed,
                "app",
                "/tmp/a.log",
                "failed",
            ),
        ];
        let previous = BTreeSet::from(["MissingInput|app|/tmp/a.log|missing".to_string()]);

        let filtered = filter_new_failures(&failures, &previous);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].detail, "failed");
    }

    #[test]
    fn work_state_changes_reports_enter_exit_once_and_skips_middle_ticks() {
        let empty = BTreeSet::new();

        // 进入暂停：当前有、上次没有 → paused 一次。
        let entered = work_state_changes(
            &empty,
            &[work_state("app", WorkState::Paused, "over")],
            &BTreeSet::from(["app".to_string()]),
        );
        assert_eq!(entered.len(), 1);
        assert_eq!(entered[0].state, WorkState::Paused);
        assert_eq!(entered[0].input_id, "app");

        // 中间 tick：仍暂停，不重复产生 paused。
        let previous = BTreeSet::from(["app".to_string()]);
        let middle = work_state_changes(
            &previous,
            &[work_state("app", WorkState::Paused, "over")],
            &previous,
        );
        assert!(middle.is_empty());

        // 恢复：上次有、当前没有 → resumed 一次。
        let resumed = work_state_changes(&previous, &[], &empty);
        assert_eq!(resumed.len(), 1);
        assert_eq!(resumed[0].state, WorkState::Resumed);
        assert_eq!(resumed[0].input_id, "app");
    }

    #[test]
    fn paused_input_signatures_key_on_input_id() {
        let signatures = paused_input_signatures(&[
            work_state("app", WorkState::Paused, "over"),
            work_state("db", WorkState::Paused, "over"),
        ]);

        assert_eq!(
            signatures,
            BTreeSet::from(["app".to_string(), "db".to_string()])
        );
    }

    #[test]
    fn work_state_changes_reports_mixed_enter_and_exit_in_same_tick() {
        let previous = BTreeSet::from(["resuming".to_string()]);
        let current_notifications = [work_state("pausing", WorkState::Paused, "over")];
        let current = BTreeSet::from(["pausing".to_string()]);

        let changes = work_state_changes(&previous, &current_notifications, &current);

        // 同一 tick：一个恢复 + 一个进入暂停。
        assert_eq!(changes.len(), 2);
        assert!(
            changes.iter().any(|change| {
                change.input_id == "resuming" && change.state == WorkState::Resumed
            })
        );
        assert!(
            changes.iter().any(|change| {
                change.input_id == "pausing" && change.state == WorkState::Paused
            })
        );
    }

    #[test]
    fn work_state_changes_returns_empty_when_no_paused_inputs() {
        let changes = work_state_changes(&BTreeSet::new(), &[], &BTreeSet::new());
        assert!(changes.is_empty());
    }
}
