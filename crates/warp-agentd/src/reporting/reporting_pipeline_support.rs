use std::io;
use std::path::Path;

use crate::error::RuntimeResult;
use crate::fs_async::read_json_async;
use wist_contracts::gateway::{ReportActionResult, ResultAttestation};
use wist_shared::fs::read_json;
use wist_shared::integrity::{dev_placeholder_issuer, digest_json, sign_dev_placeholder};
use wist_shared::time::now_rfc3339;
use wist_validate::gateway::validate_report_action_result;

use crate::reporting_pipeline::{
    PreparedReport, PreparedReportOrigin, ReportingRequest, envelope_path_for,
};
use crate::state_store::reporting::{self, ReportingState};

#[derive(Debug, Clone)]
pub(super) enum LocalReportInspection {
    Ready(Box<PreparedReport>),
    MissingState,
    CorruptState,
    MissingEnvelope(Box<ReportingState>),
    CorruptEnvelope(Box<ReportingState>),
}

pub(super) fn build_report_envelope(
    request: &ReportingRequest<'_>,
    action_id: &str,
    plan_digest: &str,
    report_attempt: u32,
    result_digest: Option<String>,
    result_signature: Option<String>,
) -> RuntimeResult<(ReportActionResult, String, String)> {
    let result_digest = result_digest.unwrap_or(digest_json(request.result)?);
    let result_signature =
        result_signature.unwrap_or_else(|| sign_dev_placeholder(request.agent_id, &result_digest));
    let reported_at = now_rfc3339();
    let attestation = ResultAttestation {
        result_digest: result_digest.clone(),
        signature: result_signature.clone(),
        issued_by: dev_placeholder_issuer(request.agent_id),
        attested_at: reported_at.clone(),
    };
    let envelope = ReportActionResult::new(
        format!("rep_{}_{}", request.execution_id, report_attempt),
        action_id.to_string(),
        report_attempt,
        request.result.final_status,
        request.execution_id.to_string(),
        plan_digest.to_string(),
        request.agent_id.to_string(),
        request.instance_id.to_string(),
        attestation,
        reported_at,
        request.result.clone(),
    );
    validate_report_action_result(&envelope)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.code))?;
    Ok((envelope, result_digest, result_signature))
}

pub(super) fn inspect_local_report(
    state_dir: &Path,
    execution_id: &str,
) -> RuntimeResult<LocalReportInspection> {
    let state_path = reporting::path_for(state_dir, execution_id);
    if !state_path.exists() {
        return Ok(LocalReportInspection::MissingState);
    }

    let state = match reporting::load(&state_path) {
        Ok(state) => state,
        Err(_) => return Ok(LocalReportInspection::CorruptState),
    };
    let envelope_path = envelope_path_for(state_dir, execution_id);
    if !envelope_path.exists() {
        return Ok(LocalReportInspection::MissingEnvelope(Box::new(state)));
    }

    let envelope = match read_json(&envelope_path) {
        Ok(envelope) => envelope,
        Err(_) => return Ok(LocalReportInspection::CorruptEnvelope(Box::new(state))),
    };
    Ok(LocalReportInspection::Ready(Box::new(PreparedReport {
        envelope_path,
        envelope,
        state,
        origin: PreparedReportOrigin::Existing,
    })))
}

pub(super) async fn inspect_local_report_async(
    state_dir: &Path,
    execution_id: &str,
) -> RuntimeResult<LocalReportInspection> {
    let state_path = reporting::path_for(state_dir, execution_id);
    match tokio::fs::metadata(&state_path).await {
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(LocalReportInspection::MissingState);
        }
        Err(err) => return Err(err.into()),
    }

    let state = match reporting::load_async(&state_path).await {
        Ok(state) => state,
        Err(_) => return Ok(LocalReportInspection::CorruptState),
    };
    let envelope_path = envelope_path_for(state_dir, execution_id);
    match tokio::fs::metadata(&envelope_path).await {
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(LocalReportInspection::MissingEnvelope(Box::new(state)));
        }
        Err(err) => return Err(err.into()),
    }

    let envelope = match read_json_async(&envelope_path).await {
        Ok(envelope) => envelope,
        Err(_) => return Ok(LocalReportInspection::CorruptEnvelope(Box::new(state))),
    };
    Ok(LocalReportInspection::Ready(Box::new(PreparedReport {
        envelope_path,
        envelope,
        state,
        origin: PreparedReportOrigin::Existing,
    })))
}

pub(super) fn sync_reporting_state(
    state_dir: &Path,
    execution_id: &str,
    state: &ReportingState,
    envelope_path: &Path,
    result_digest: &str,
    result_signature: &str,
) -> RuntimeResult<ReportingState> {
    let mut rebuilt_state = state.clone();
    let mut state_changed = false;
    let envelope_path_str = envelope_path.display().to_string();
    if rebuilt_state.report_envelope_path.as_deref() != Some(envelope_path_str.as_str()) {
        rebuilt_state.report_envelope_path = Some(envelope_path_str);
        state_changed = true;
    }
    if rebuilt_state.result_digest.as_deref() != Some(result_digest) {
        rebuilt_state.result_digest = Some(result_digest.to_string());
        state_changed = true;
    }
    if rebuilt_state.result_signature.as_deref() != Some(result_signature) {
        rebuilt_state.result_signature = Some(result_signature.to_string());
        state_changed = true;
    }
    if state_changed {
        let state_path = reporting::path_for(state_dir, execution_id);
        reporting::store(&state_path, &rebuilt_state)?;
    }
    Ok(rebuilt_state)
}

pub(super) async fn sync_reporting_state_async(
    state_dir: &Path,
    execution_id: &str,
    state: &ReportingState,
    envelope_path: &Path,
    result_digest: &str,
    result_signature: &str,
) -> RuntimeResult<ReportingState> {
    let mut rebuilt_state = state.clone();
    let mut state_changed = false;
    let envelope_path_str = envelope_path.display().to_string();
    if rebuilt_state.report_envelope_path.as_deref() != Some(envelope_path_str.as_str()) {
        rebuilt_state.report_envelope_path = Some(envelope_path_str);
        state_changed = true;
    }
    if rebuilt_state.result_digest.as_deref() != Some(result_digest) {
        rebuilt_state.result_digest = Some(result_digest.to_string());
        state_changed = true;
    }
    if rebuilt_state.result_signature.as_deref() != Some(result_signature) {
        rebuilt_state.result_signature = Some(result_signature.to_string());
        state_changed = true;
    }
    if state_changed {
        let state_path = reporting::path_for(state_dir, execution_id);
        reporting::store_async(&state_path, &rebuilt_state).await?;
    }
    Ok(rebuilt_state)
}
