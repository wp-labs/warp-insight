//! Structured record sinks for local `warp-parse` style output.

use std::io;
use std::path::PathBuf;

use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use wist_contracts::agent_config::LogsOutputSection;
use wist_contracts::telemetry_record::{DataFrame, TelemetryRecordContract};
use wist_shared::fs::ensure_parent;

pub(crate) trait RecordSink {
    async fn write_records(&mut self, records: &[TelemetryRecordContract]) -> io::Result<()>;
}

impl<T> RecordSink for &mut T
where
    T: RecordSink + ?Sized,
{
    async fn write_records(&mut self, records: &[TelemetryRecordContract]) -> io::Result<()> {
        (**self).write_records(records).await
    }
}

#[derive(Debug)]
pub(crate) enum TelemetryRecordSink {
    File(FileRecordSink),
    Tcp(TcpRecordSink),
}

impl RecordSink for TelemetryRecordSink {
    async fn write_records(&mut self, records: &[TelemetryRecordContract]) -> io::Result<()> {
        match self {
            Self::File(sink) => sink.write_records(records).await,
            Self::Tcp(sink) => sink.write_records(records).await,
        }
    }
}

impl TelemetryRecordSink {
    pub(crate) fn from_logs_output(output: &LogsOutputSection) -> io::Result<Self> {
        match output.kind.as_str() {
            "file" => Ok(Self::File(FileRecordSink::new(PathBuf::from(
                &output.file.path,
            )))),
            "tcp" => Ok(Self::Tcp(TcpRecordSink::new(
                output.tcp.addr.clone(),
                output.tcp.port,
                TcpFraming::parse(&output.tcp.framing)?,
            ))),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported telemetry output kind: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub(crate) struct FileRecordSink {
    path: PathBuf,
}

impl FileRecordSink {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl RecordSink for FileRecordSink {
    async fn write_records(&mut self, records: &[TelemetryRecordContract]) -> io::Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        ensure_parent(&self.path)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        for record in records {
            let encoded = serde_json::to_vec(record).map_err(io::Error::other)?;
            file.write_all(&encoded).await?;
            file.write_all(b"\n").await?;
        }
        file.sync_all().await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TcpFraming {
    Line,
    Len,
}

impl TcpFraming {
    pub(crate) fn parse(raw: &str) -> io::Result<Self> {
        match raw {
            "line" => Ok(Self::Line),
            "len" => Ok(Self::Len),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported tcp framing: {raw}"),
            )),
        }
    }
}

#[derive(Debug, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub(crate) struct TcpRecordSink {
    target_addr: String,
    framing: TcpFraming,
    stream: Option<TcpStream>,
}

impl TcpRecordSink {
    pub(crate) fn new(addr: String, port: u16, framing: TcpFraming) -> Self {
        Self {
            target_addr: format!("{addr}:{port}"),
            framing,
            stream: None,
        }
    }

    async fn stream(&mut self) -> io::Result<&mut TcpStream> {
        if self.stream.is_none() {
            self.stream = Some(TcpStream::connect(&self.target_addr).await?);
        }
        Ok(self.stream.as_mut().expect("stream initialized"))
    }
}

impl RecordSink for TcpRecordSink {
    async fn write_records(&mut self, records: &[TelemetryRecordContract]) -> io::Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let mut payload = Vec::new();
        for record in records {
            let frame = build_record_frame(record)?;
            payload.extend_from_slice(&build_payload_bytes(&frame, self.framing));
        }

        match self.stream().await?.write_all(&payload).await {
            Ok(()) => Ok(()),
            Err(err) => {
                self.stream = None;
                Err(err)
            }
        }
    }
}

/// TCP 上送帧：结构化信封（不含原文）与 `RAW:` 原始行分离，避免把 raw 塞进 JSON。
///
/// `{envelope} RAW: <body>`，其中 envelope 只承载通用字段（`schema`/`agent`/`ts`/`seq`，短名），
/// body 保持原文、不转义，供数据面审计核对与回放。来源细节（`input`/路径/偏移）不进帧。
fn build_record_frame(record: &TelemetryRecordContract) -> io::Result<Vec<u8>> {
    let envelope = DataFrame::from(record);
    let mut frame = serde_json::to_vec(&envelope)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    // RAW: 后跟一个空格分隔帧标记与原文，保证原文从正文首字符开始、不带标记前缀。
    frame.extend_from_slice(b" RAW: ");
    frame.extend_from_slice(record.body.as_bytes());
    Ok(frame)
}

fn build_payload_bytes(data: &[u8], framing: TcpFraming) -> Vec<u8> {
    match framing {
        TcpFraming::Line => {
            if data.last() == Some(&b'\n') {
                data.to_vec()
            } else {
                let mut buf = Vec::with_capacity(data.len() + 1);
                buf.extend_from_slice(data);
                buf.push(b'\n');
                buf
            }
        }
        TcpFraming::Len => {
            let mut buf = Vec::with_capacity(16 + data.len());
            buf.extend_from_slice(data.len().to_string().as_bytes());
            buf.push(b' ');
            buf.extend_from_slice(data);
            buf
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::time::{SystemTime, UNIX_EPOCH};

    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    use super::{FileRecordSink, RecordSink, TcpFraming, TcpRecordSink, build_payload_bytes};
    use wist_contracts::telemetry_record::TelemetryRecordContract;

    fn record(body: &str) -> TelemetryRecordContract {
        TelemetryRecordContract::new_log(
            "agent-a".to_string(),
            "2026-04-14T00:00:00Z".to_string(),
            "input-a".to_string(),
            "/tmp/app.log".to_string(),
            body.to_string(),
            0,
            body.len() as u64,
            0,
        )
    }

    fn temp_file(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        std::env::temp_dir().join(format!("warp-agentd-warp-parse-{name}-{suffix}.ndjson"))
    }

    #[tokio::test(flavor = "current_thread")]
    async fn file_record_sink_writes_ndjson() {
        let path = temp_file("file-sink");
        let mut sink = FileRecordSink::new(path.clone());

        sink.write_records(&[record("a"), record("b")])
            .await
            .expect("write records");

        let written = fs::read_to_string(&path).expect("read output");
        assert!(written.contains("\"body\":\"a\""));
        assert!(written.contains("\"body\":\"b\""));
        fs::remove_file(path).ok();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn tcp_record_sink_sends_envelope_and_raw_frame() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return,
            Err(err) => panic!("bind listener: {err}"),
        };
        let port = listener.local_addr().expect("listener addr").port();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buf = vec![0u8; 1024];
            let n = socket.read(&mut buf).await.expect("read");
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        let mut sink = TcpRecordSink::new("127.0.0.1".to_string(), port, TcpFraming::Line);

        sink.write_records(&[record("line-a"), record("line-b")])
            .await
            .expect("write records");

        let body = server.await.expect("join");
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 2);
        for line in &lines {
            let (envelope, raw) = line.split_once(" RAW: ").expect("RAW marker");
            assert!(envelope.starts_with('{'), "envelope json: {envelope}");
            assert!(envelope.contains("\"agent\":\"agent-a\""));
            assert!(
                !envelope.contains("\"body\""),
                "raw must not be in envelope"
            );
            assert!(raw.starts_with("line-"), "raw body: {raw}");
        }
    }

    #[test]
    fn record_frame_keeps_raw_outside_json_envelope() {
        let frame = super::build_record_frame(&record("raw 行内容")).expect("build frame");
        let text = String::from_utf8_lossy(&frame);
        let (envelope, raw) = text.split_once(" RAW: ").expect("RAW marker");
        let parsed: serde_json::Value = serde_json::from_str(envelope).expect("valid envelope");
        assert_eq!(parsed["schema"], "v1");
        assert_eq!(parsed["agent"], "agent-a");
        assert_eq!(parsed["ts"], "2026-04-14T00:00:00Z");
        assert_eq!(parsed["seq"], 0);
        assert!(parsed.get("body").is_none(), "raw must not be in envelope");
        assert!(
            parsed.get("input_id").is_none(),
            "input_id must not be in envelope"
        );
        assert!(
            parsed.get("signal_kind").is_none(),
            "signal_kind must not be in envelope"
        );
        assert_eq!(raw, "raw 行内容");
    }

    #[test]
    fn payload_builder_matches_line_and_len_contract() {
        assert_eq!(build_payload_bytes(b"abc", TcpFraming::Line), b"abc\n");
        assert_eq!(build_payload_bytes(b"hello", TcpFraming::Len), b"5 hello");
    }
}
