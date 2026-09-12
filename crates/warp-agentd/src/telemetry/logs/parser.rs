//! Conversion from folded lines into structured telemetry records.

use wist_contracts::telemetry_record::TelemetryRecordContract;

use super::multiline::FoldedLine;

pub fn parse_folded_lines(
    agent_id: &str,
    observed_at: &str,
    input_id: &str,
    source_path: &str,
    lines: Vec<FoldedLine>,
    next_seq: &mut u64,
) -> Vec<TelemetryRecordContract> {
    lines
        .into_iter()
        .map(|line| {
            let seq = *next_seq;
            *next_seq += 1;
            TelemetryRecordContract::new_log(
                agent_id.to_string(),
                observed_at.to_string(),
                input_id.to_string(),
                source_path.to_string(),
                line.body,
                line.start_offset,
                line.end_offset,
                seq,
            )
        })
        .collect()
}
