//! In-memory staging for telemetry records before sink or spool.

use warp_insight_contracts::telemetry_record::TelemetryRecordContract;

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct StageResult {
    pub staged: Vec<TelemetryRecordContract>,
    pub overflowed: Vec<TelemetryRecordContract>,
}

#[derive(Debug, Clone, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct TelemetryBuffer {
    max_bytes: usize,
    used_bytes: usize,
    staged: Vec<TelemetryRecordContract>,
}

impl TelemetryBuffer {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            used_bytes: 0,
            staged: Vec::new(),
        }
    }

    /// Stages records until the in-memory byte budget is exhausted.
    ///
    /// Records that do not fit are returned in `overflowed`; callers are
    /// responsible for spooling or otherwise handling them.
    pub fn stage_all(&mut self, records: Vec<TelemetryRecordContract>) -> StageResult {
        let mut overflowed = Vec::new();
        for record in records {
            let record_size = estimate_record_size(&record);
            if self.used_bytes + record_size > self.max_bytes {
                overflowed.push(record);
                continue;
            }
            self.used_bytes += record_size;
            self.staged.push(record);
        }
        StageResult {
            staged: std::mem::take(&mut self.staged),
            overflowed,
        }
    }
}

fn estimate_record_size(record: &TelemetryRecordContract) -> usize {
    record.body.len()
        + record.input_id.len()
        + record.source_path.len()
        + record.signal_kind.len()
        + 64
}

#[cfg(test)]
mod tests {
    use super::TelemetryBuffer;
    use warp_insight_contracts::telemetry_record::TelemetryRecordContract;

    fn record(body: &str) -> TelemetryRecordContract {
        TelemetryRecordContract::new_log(
            "2026-04-13T00:00:00Z".to_string(),
            "input-a".to_string(),
            "/tmp/app.log".to_string(),
            body.to_string(),
            0,
            body.len() as u64,
        )
    }

    #[test]
    fn stage_all_overflows_after_first_staged_record_when_budget_is_exceeded() {
        let mut buffer = TelemetryBuffer::new(120);

        let staged = buffer.stage_all(vec![record("line-1"), record("line-2"), record("line-3")]);

        assert_eq!(staged.staged.len(), 1);
        assert_eq!(staged.overflowed.len(), 2);
    }

    #[test]
    fn stage_all_spools_single_record_that_alone_exceeds_budget() {
        let mut buffer = TelemetryBuffer::new(16);

        let staged = buffer.stage_all(vec![record("this line is much too large")]);

        assert!(staged.staged.is_empty());
        assert_eq!(staged.overflowed.len(), 1);
    }
}
