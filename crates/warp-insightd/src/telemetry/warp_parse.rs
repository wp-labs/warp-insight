//! Structured record sinks for local `warp-parse` style output.

use std::io;
use std::path::PathBuf;

use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use warp_insight_contracts::agent_config::LogsOutputSection;
use warp_insight_contracts::telemetry_record::TelemetryRecordContract;
use warp_insight_shared::fs::ensure_parent;

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

#[derive(Debug, Clone, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
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

#[derive(Debug, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
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
            payload.extend_from_slice(&build_payload_bytes(record.body.as_bytes(), self.framing));
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
    use warp_insight_contracts::telemetry_record::TelemetryRecordContract;

    fn record(body: &str) -> TelemetryRecordContract {
        TelemetryRecordContract::new_log(
            "2026-04-14T00:00:00Z".to_string(),
            "input-a".to_string(),
            "/tmp/app.log".to_string(),
            body.to_string(),
            0,
            body.len() as u64,
        )
    }

    fn temp_file(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        std::env::temp_dir().join(format!("warp-insightd-warp-parse-{name}-{suffix}.ndjson"))
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
    async fn tcp_record_sink_sends_line_framed_bodies() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return,
            Err(err) => panic!("bind listener: {err}"),
        };
        let port = listener.local_addr().expect("listener addr").port();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buf = vec![0u8; 64];
            let n = socket.read(&mut buf).await.expect("read");
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        let mut sink = TcpRecordSink::new("127.0.0.1".to_string(), port, TcpFraming::Line);

        sink.write_records(&[record("line-a"), record("line-b")])
            .await
            .expect("write records");

        let body = server.await.expect("join");
        assert_eq!(body, "line-a\nline-b\n");
    }

    #[test]
    fn payload_builder_matches_line_and_len_contract() {
        assert_eq!(build_payload_bytes(b"abc", TcpFraming::Line), b"abc\n");
        assert_eq!(build_payload_bytes(b"hello", TcpFraming::Len), b"5 hello");
    }
}
