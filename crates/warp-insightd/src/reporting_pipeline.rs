//! Local result reporting preparation.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use warp_insight_contracts::action_result::ActionResultContract;
use warp_insight_contracts::gateway::ReportActionResult;
use warp_insight_shared::fs::write_json_atomic;
use warp_insight_shared::paths::REPORT_ENVELOPE_SUFFIX;

use crate::state_store::reporting::{self, ReportingState};

#[path = "reporting_pipeline_support.rs"]
mod support;

use support::{
    LocalReportInspection, build_report_envelope, inspect_local_report, sync_reporting_state,
};

#[derive(Debug, Clone, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
pub struct ReportingRequest<'a> {
    pub state_dir: &'a Path,
    pub execution_id: &'a str,
    pub action_id: &'a str,
    pub request_id: &'a str,
    pub plan_digest: &'a str,
    pub agent_id: &'a str,
    pub instance_id: &'a str,
    pub final_state: &'a str,
    pub result_path: &'a Path,
    pub result: &'a ActionResultContract,
}

#[derive(Debug, Clone, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
pub struct PreparedReport {
    pub envelope_path: PathBuf,
    pub envelope: ReportActionResult,
    pub state: ReportingState,
    pub origin: PreparedReportOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedReportOrigin {
    Existing,
    Prepared(LocalReportIssue),
    EnvelopeRebuilt(LocalReportIssue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalReportIssue {
    NewReport,
    MissingState,
    CorruptState,
    MissingEnvelope,
    CorruptEnvelope,
    ManualRebuild,
}

pub fn load_complete_local_report(
    state_dir: &Path,
    execution_id: &str,
) -> io::Result<Option<PreparedReport>> {
    Ok(match inspect_local_report(state_dir, execution_id)? {
        LocalReportInspection::Ready(prepared) => Some(*prepared),
        LocalReportInspection::MissingState
        | LocalReportInspection::CorruptState
        | LocalReportInspection::MissingEnvelope(_)
        | LocalReportInspection::CorruptEnvelope(_) => None,
    })
}

pub fn ensure_local_report(request: ReportingRequest<'_>) -> io::Result<PreparedReport> {
    match inspect_local_report(request.state_dir, request.execution_id)? {
        LocalReportInspection::Ready(prepared) => Ok(*prepared),
        LocalReportInspection::MissingState => {
            prepare_local_report_with_issue(request, LocalReportIssue::MissingState)
        }
        LocalReportInspection::CorruptState => {
            prepare_local_report_with_issue(request, LocalReportIssue::CorruptState)
        }
        LocalReportInspection::MissingEnvelope(state) => {
            rebuild_report_envelope_with_issue(request, &state, LocalReportIssue::MissingEnvelope)
        }
        LocalReportInspection::CorruptEnvelope(state) => {
            rebuild_report_envelope_with_issue(request, &state, LocalReportIssue::CorruptEnvelope)
        }
    }
}

pub fn prepare_local_report(request: ReportingRequest<'_>) -> io::Result<PreparedReport> {
    prepare_local_report_with_issue(request, LocalReportIssue::NewReport)
}

fn prepare_local_report_with_issue(
    request: ReportingRequest<'_>,
    issue: LocalReportIssue,
) -> io::Result<PreparedReport> {
    let (envelope, result_digest, result_signature) = build_report_envelope(
        &request,
        request.action_id,
        request.plan_digest,
        1,
        None,
        None,
    )?;

    let envelope_path = envelope_path_for(request.state_dir, request.execution_id);
    write_json_atomic(&envelope_path, &envelope)?;

    let state = ReportingState::new(
        request.execution_id.to_string(),
        request.action_id.to_string(),
        request.plan_digest.to_string(),
        request.request_id.to_string(),
        request.final_state.to_string(),
        request.result_path.display().to_string(),
        Some(envelope_path.display().to_string()),
        Some(result_digest),
        Some(result_signature),
        0,
        None,
        None,
    );
    let state_path = reporting::path_for(request.state_dir, request.execution_id);
    reporting::store(&state_path, &state)?;

    Ok(PreparedReport {
        envelope_path,
        envelope,
        state,
        origin: PreparedReportOrigin::Prepared(issue),
    })
}

pub fn rebuild_report_envelope(
    request: ReportingRequest<'_>,
    state: &ReportingState,
) -> io::Result<PreparedReport> {
    rebuild_report_envelope_with_issue(request, state, LocalReportIssue::ManualRebuild)
}

fn rebuild_report_envelope_with_issue(
    request: ReportingRequest<'_>,
    state: &ReportingState,
    issue: LocalReportIssue,
) -> io::Result<PreparedReport> {
    let (envelope, result_digest, result_signature) = build_report_envelope(
        &request,
        &state.action_id,
        &state.plan_digest,
        state.report_attempt.saturating_add(1),
        state.result_digest.clone(),
        state.result_signature.clone(),
    )?;

    let envelope_path = envelope_path_for(request.state_dir, request.execution_id);
    write_json_atomic(&envelope_path, &envelope)?;

    let rebuilt_state = sync_reporting_state(
        request.state_dir,
        request.execution_id,
        state,
        &envelope_path,
        &result_digest,
        &result_signature,
    )?;

    Ok(PreparedReport {
        envelope_path,
        envelope,
        state: rebuilt_state,
        origin: PreparedReportOrigin::EnvelopeRebuilt(issue),
    })
}

pub fn envelope_path_for(state_dir: &Path, execution_id: &str) -> PathBuf {
    state_dir
        .join("reporting")
        .join(format!("{execution_id}{REPORT_ENVELOPE_SUFFIX}"))
}

pub fn remove_local_report_artifacts(state_dir: &Path, execution_id: &str) -> io::Result<()> {
    for path in [
        reporting::path_for(state_dir, execution_id),
        envelope_path_for(state_dir, execution_id),
    ] {
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "reporting_pipeline_tests.rs"]
mod tests;
