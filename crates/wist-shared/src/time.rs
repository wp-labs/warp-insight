//! Shared time helpers.

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("format RFC3339 timestamp")
}

/// 当前 Unix 时间戳（毫秒）。
pub fn now_ts_ms() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp_nanos() as i64 / 1_000_000
}

pub fn after_millis_rfc3339(millis: u64) -> String {
    (OffsetDateTime::now_utc() + time::Duration::milliseconds(millis as i64))
        .format(&Rfc3339)
        .expect("format RFC3339 timestamp")
}
