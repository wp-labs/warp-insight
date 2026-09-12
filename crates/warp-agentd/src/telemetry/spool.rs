//! Durable local spool for structured telemetry records.

#[cfg(test)]
use std::fs;
use std::io;
use std::path::Path;

use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use wist_contracts::telemetry_record::TelemetryRecordContract;
use wist_shared::fs::ensure_parent;

use crate::telemetry::warp_parse::RecordSink;

pub async fn append_records_async(
    path: &Path,
    records: &[TelemetryRecordContract],
) -> io::Result<()> {
    if records.is_empty() {
        return Ok(());
    }

    ensure_parent(path)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    for record in records {
        let encoded = serde_json::to_vec(record).map_err(io::Error::other)?;
        file.write_all(&encoded).await?;
        file.write_all(b"\n").await?;
    }
    file.sync_all().await?;
    Ok(())
}

#[cfg(test)]
pub fn append_records(path: &Path, records: &[TelemetryRecordContract]) -> io::Result<()> {
    block_on_io(append_records_async(path, records))
}

#[cfg(test)]
pub fn load_records(path: &Path) -> io::Result<Vec<TelemetryRecordContract>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(parse_record_line)
        .collect()
}

pub async fn has_records_async(path: &Path) -> io::Result<bool> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) => Ok(metadata.len() > 0),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
pub fn has_records(path: &Path) -> io::Result<bool> {
    block_on_io(has_records_async(path))
}

/// spool 当前字节数；文件不存在视为 0，用于上限（背压）判断。
pub async fn size_async(path: &Path) -> io::Result<u64> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) => Ok(metadata.len()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
pub fn size(path: &Path) -> io::Result<u64> {
    block_on_io(size_async(path))
}

pub async fn replay_records_async<S: RecordSink>(
    path: &Path,
    sink: &mut S,
    batch_size: usize,
) -> io::Result<usize> {
    let file = match File::open(path).await {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err),
    };

    let mut reader = BufReader::new(file);
    let mut replayed = 0usize;
    let mut batch = Vec::with_capacity(batch_size.max(1));
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        batch.push(parse_record_line(&line)?);
        if batch.len() >= batch.capacity() {
            sink.write_records(&batch).await?;
            replayed += batch.len();
            batch.clear();
        }
    }

    if !batch.is_empty() {
        sink.write_records(&batch).await?;
        replayed += batch.len();
    }
    clear_async(path).await?;
    Ok(replayed)
}

#[cfg(test)]
pub fn replay_records<S: RecordSink>(
    path: &Path,
    sink: &mut S,
    batch_size: usize,
) -> io::Result<usize> {
    block_on_io(replay_records_async(path, sink, batch_size))
}

pub async fn clear_async(path: &Path) -> io::Result<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
pub fn clear(path: &Path) -> io::Result<()> {
    block_on_io(clear_async(path))
}

fn parse_record_line(line: &str) -> io::Result<TelemetryRecordContract> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    serde_json::from_str::<TelemetryRecordContract>(trimmed).map_err(io::Error::other)
}

#[cfg(test)]
fn block_on_io<T>(future: impl std::future::Future<Output = io::Result<T>>) -> io::Result<T> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(future)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        append_records, append_records_async, clear, has_records, has_records_async, load_records,
        replay_records, replay_records_async, size,
    };
    use crate::telemetry::warp_parse::RecordSink;
    use wist_contracts::telemetry_record::TelemetryRecordContract;

    fn temp_file(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        std::env::temp_dir().join(format!("warp-agentd-spool-{name}-{suffix}.ndjson"))
    }

    fn record(body: &str) -> TelemetryRecordContract {
        TelemetryRecordContract::new_log(
            "agent-a".to_string(),
            "2026-04-13T00:00:00Z".to_string(),
            "input-a".to_string(),
            "/tmp/app.log".to_string(),
            body.to_string(),
            0,
            body.len() as u64,
            0,
        )
    }

    #[test]
    fn append_and_load_round_trip_records() {
        let path = temp_file("round-trip");
        append_records(&path, &[record("a"), record("b")]).expect("append");

        let loaded = load_records(&path).expect("load");

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].body, "a");
        assert_eq!(loaded[1].body, "b");
        fs::remove_file(path).ok();
    }

    #[test]
    fn clear_removes_spool_file() {
        let path = temp_file("clear");
        append_records(&path, &[record("a")]).expect("append");

        clear(&path).expect("clear");

        assert!(!path.exists());
    }

    #[test]
    fn size_is_zero_when_missing_and_bytes_when_present() {
        let path = temp_file("size");

        assert_eq!(size(&path).expect("size missing"), 0);

        append_records(&path, &[record("a")]).expect("append");

        assert!(size(&path).expect("size present") > 0);
        fs::remove_file(path).ok();
    }

    #[derive(Default)]
    struct TestSink {
        records: Vec<TelemetryRecordContract>,
        fail_after_batches: Option<usize>,
        batches: usize,
    }

    impl RecordSink for TestSink {
        async fn write_records(&mut self, records: &[TelemetryRecordContract]) -> io::Result<()> {
            self.batches += 1;
            if self
                .fail_after_batches
                .is_some_and(|limit| self.batches > limit)
            {
                return Err(io::Error::other("sink unavailable"));
            }
            self.records.extend_from_slice(records);
            Ok(())
        }
    }

    #[test]
    fn replay_records_streams_batches_and_clears_spool_on_success() {
        let path = temp_file("replay");
        append_records(&path, &[record("a"), record("b"), record("c")]).expect("append");
        let mut sink = TestSink::default();

        let replayed = replay_records(&path, &mut sink, 2).expect("replay");

        assert_eq!(replayed, 3);
        assert_eq!(sink.records.len(), 3);
        assert!(!has_records(&path).expect("spool presence"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn replay_records_leaves_spool_when_sink_fails() {
        let path = temp_file("replay-fail");
        append_records_async(&path, &[record("a"), record("b"), record("c")])
            .await
            .expect("append");
        let mut sink = TestSink {
            fail_after_batches: Some(1),
            ..Default::default()
        };

        let err = replay_records_async(&path, &mut sink, 2)
            .await
            .expect_err("replay should fail");

        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert!(has_records_async(&path).await.expect("spool presence"));
        fs::remove_file(path).ok();
    }
}
