//! Structured record sinks for local `warp-parse` style output.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::{Instant, timeout};
use wist_contracts::agent_config::LogsOutputSection;
use wist_contracts::telemetry_record::{DataFrame, TelemetryRecordContract};
use wist_shared::fs::ensure_parent;

use crate::telemetry::metrics::samples::VmMetricLine;

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

    /// 上送一帧指标帧。指标走 TCP uplink，与日志复用同一连接、靠 ` METRICS:` 帧标记区分；
    /// 文件输出仅承载日志（本地调试），不承载指标，返回 Ok 跳过。
    pub(crate) async fn write_metrics(
        &mut self,
        envelope: &DataFrame,
        metrics: &VmMetricLine,
    ) -> io::Result<()> {
        match self {
            Self::File(_) => Ok(()),
            Self::Tcp(sink) => sink.write_metrics(envelope, metrics).await,
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

/// TCP 连接超时：避免死网卡/黑洞地址让 tick 永久挂起。
const TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// TCP 写超时：避免对端不读导致的写永久挂起。
const TCP_WRITE_TIMEOUT: Duration = Duration::from_secs(5);
/// 退避初始时长与上限（失败翻倍、成功重置）。
const TCP_BACKOFF_INITIAL: Duration = Duration::from_secs(1);
const TCP_BACKOFF_MAX: Duration = Duration::from_secs(30);

#[derive(Debug, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub(crate) struct TcpRecordSink {
    target_addr: String,
    framing: TcpFraming,
    stream: Option<TcpStream>,
    /// 退避窗口内不允许再尝试连接的时间点（指数退避）。
    next_attempt_at: Option<Instant>,
    /// 当前退避时长：失败翻倍、成功重置为初始值。
    backoff: Duration,
}

impl TcpRecordSink {
    pub(crate) fn new(addr: String, port: u16, framing: TcpFraming) -> Self {
        Self {
            target_addr: format!("{addr}:{port}"),
            framing,
            stream: None,
            next_attempt_at: None,
            backoff: TCP_BACKOFF_INITIAL,
        }
    }

    fn in_backoff(&self) -> bool {
        self.next_attempt_at.is_some_and(|at| Instant::now() < at)
    }

    fn schedule_backoff(&mut self) {
        self.backoff = self.backoff.saturating_mul(2).min(TCP_BACKOFF_MAX);
        self.next_attempt_at = Some(Instant::now() + self.backoff);
    }

    fn backoff_error() -> io::Error {
        io::Error::new(io::ErrorKind::WouldBlock, "tcp uplink in backoff")
    }

    /// 确保连接可用：无连接则先 connect（带超时）；退避窗口内直接失败交给 spool。
    async fn ensure_connected(&mut self) -> io::Result<()> {
        if self.stream.is_some() {
            return Ok(());
        }
        if self.in_backoff() {
            return Err(Self::backoff_error());
        }
        match timeout(TCP_CONNECT_TIMEOUT, TcpStream::connect(&self.target_addr)).await {
            Ok(Ok(stream)) => {
                self.stream = Some(stream);
                self.backoff = TCP_BACKOFF_INITIAL;
                self.next_attempt_at = None;
                Ok(())
            }
            Ok(Err(err)) => {
                self.schedule_backoff();
                Err(err)
            }
            Err(_) => {
                self.schedule_backoff();
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "tcp connect timed out",
                ))
            }
        }
    }

    /// 写一段已分帧的字节到连接；失败时重置连接并进入指数退避，以便下次重连。
    async fn write_payload(&mut self, payload: &[u8]) -> io::Result<()> {
        self.ensure_connected().await?;
        let write = {
            let stream = self.stream.as_mut().expect("connected");
            timeout(TCP_WRITE_TIMEOUT, stream.write_all(payload)).await
        };
        match write {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => {
                self.stream = None;
                self.schedule_backoff();
                Err(err)
            }
            Err(_) => {
                self.stream = None;
                self.schedule_backoff();
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "tcp write timed out",
                ))
            }
        }
    }

    /// 写入一帧指标帧（` METRICS: ` 标记 + 结构化 JSON 正文）。
    pub(crate) async fn write_metrics(
        &mut self,
        envelope: &DataFrame,
        metrics: &VmMetricLine,
    ) -> io::Result<()> {
        let frame = build_metrics_frame(envelope, metrics)?;
        self.write_payload(&build_payload_bytes(&frame, self.framing))
            .await
    }

    /// 测试用：缩短退避窗口，避免真实等待默认的 1s。
    #[cfg(test)]
    fn with_backoff(mut self, initial: Duration) -> Self {
        self.backoff = initial;
        self
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

        self.write_payload(&payload).await
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

/// TCP 指标帧：与日志帧共用 `DataFrame` 信封，但帧标记为 ` METRICS: `，正文是 VM JSON line。
///
/// `{envelope} METRICS: {"metric":{...},"value":<number>}`；信封 `seq` 参与 `(agent, seq)` 去重/查缺。
fn build_metrics_frame(envelope: &DataFrame, metrics: &VmMetricLine) -> io::Result<Vec<u8>> {
    let mut frame = serde_json::to_vec(envelope)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    frame.extend_from_slice(b" METRICS: ");
    let body = serde_json::to_vec(metrics)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    frame.extend_from_slice(&body);
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
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;
    use tokio::time::Instant;

    use super::{
        FileRecordSink, RecordSink, TCP_BACKOFF_INITIAL, TCP_BACKOFF_MAX, TcpFraming,
        TcpRecordSink, build_payload_bytes,
    };

    use crate::telemetry::metrics::samples::{VmMetricLabels, VmMetricLine};
    use wist_contracts::telemetry_record::{DataFrame, TelemetryRecordContract};

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

    #[tokio::test(flavor = "current_thread")]
    async fn tcp_sink_backs_off_after_connect_failure() {
        // 绑定后立即 drop，拿到一个确定关闭的端口，connect 会立即 ECONNREFUSED。
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            listener.local_addr().expect("addr").port()
        };
        // 测试用缩短退避窗口，避免真实等待 1s。
        let mut sink = TcpRecordSink::new("127.0.0.1".to_string(), port, TcpFraming::Line)
            .with_backoff(Duration::from_millis(10));

        // 第一次 connect 失败，进入退避（10ms → 20ms）。
        assert!(sink.write_records(&[record("a")]).await.is_err());

        // 退避窗口内：快速返回 WouldBlock，不再尝试 connect。
        let err = sink
            .write_records(&[record("b")])
            .await
            .expect_err("backoff");
        assert_eq!(err.kind(), io::ErrorKind::WouldBlock);

        // 越过退避窗口：重新尝试 connect（仍失败，退避翻倍到 40ms）。
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(sink.write_records(&[record("c")]).await.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn tcp_sink_backoff_doubles_and_caps() {
        let mut sink = TcpRecordSink::new("127.0.0.1".to_string(), 1, TcpFraming::Line);

        // 1s → 2s → 4s → 8s → 16s → 30s（封顶）。
        let mut expected = TCP_BACKOFF_INITIAL;
        for _ in 0..5 {
            sink.schedule_backoff();
            expected = expected.saturating_mul(2).min(TCP_BACKOFF_MAX);
            assert_eq!(sink.backoff, expected);
        }

        // 已封顶：继续失败不再增长。
        let capped = sink.backoff;
        assert_eq!(capped, TCP_BACKOFF_MAX);
        sink.schedule_backoff();
        assert_eq!(sink.backoff, capped);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn tcp_sink_resets_backoff_on_successful_connect() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.expect("accept");
            // 保持连接一小段时间，确保客户端 connect + write 完成。
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        let mut sink = TcpRecordSink::new("127.0.0.1".to_string(), port, TcpFraming::Line);
        // 直接置为非初始退避态（窗口已过期），模拟之前失败过。
        sink.backoff = TCP_BACKOFF_MAX;
        sink.next_attempt_at = Some(Instant::now() - Duration::from_secs(1));

        sink.write_records(&[record("a")]).await.expect("write");

        // 成功连接后：退避重置为初始值、退避窗口清除。
        assert_eq!(sink.backoff, TCP_BACKOFF_INITIAL);
        assert!(sink.next_attempt_at.is_none());
        server.await.expect("server");
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

    #[test]
    fn metrics_frame_uses_metrics_marker_with_json_body() {
        let envelope = DataFrame::new("agent-001", "2026-04-14T00:00:00Z", 42);
        let metrics = VmMetricLine {
            metric: VmMetricLabels {
                name: "system.load_average.1m".to_string(),
                agent: "agent-001".to_string(),
                kind: "host_metrics".to_string(),
                target_ref: "host-1:host".to_string(),
                resource_ref: Some("host-1".to_string()),
                unit: "1".to_string(),
            },
            values: vec![0.25],
            timestamps: vec![1_234_567_890_000],
        };

        let frame = super::build_metrics_frame(&envelope, &metrics).expect("build frame");
        let text = String::from_utf8_lossy(&frame);
        let (env, body) = text.split_once(" METRICS: ").expect("METRICS marker");

        let parsed_env: serde_json::Value = serde_json::from_str(env).expect("valid envelope");
        assert_eq!(parsed_env["agent"], "agent-001");
        assert_eq!(parsed_env["seq"], 42);
        assert!(
            parsed_env.get("kind").is_none(),
            "envelope stays signal-agnostic"
        );

        let parsed_body: serde_json::Value = serde_json::from_str(body).expect("valid metrics");
        assert_eq!(parsed_body["metric"]["__name__"], "system.load_average.1m");
        assert_eq!(parsed_body["metric"]["agent"], "agent-001");
        assert_eq!(parsed_body["metric"]["kind"], "host_metrics");
        assert_eq!(parsed_body["values"][0], 0.25);
        assert_eq!(parsed_body["timestamps"][0], 1_234_567_890_000_i64);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn tcp_record_sink_writes_metrics_frame() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return,
            Err(err) => panic!("bind listener: {err}"),
        };
        let port = listener.local_addr().expect("listener addr").port();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buf = vec![0u8; 2048];
            let n = socket.read(&mut buf).await.expect("read");
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        let mut sink = TcpRecordSink::new("127.0.0.1".to_string(), port, TcpFraming::Line);

        let envelope = DataFrame::new("agent-001", "2026-04-14T00:00:00Z", 7);
        let metrics = VmMetricLine {
            metric: VmMetricLabels {
                name: "system.load_average.1m".to_string(),
                agent: "agent-001".to_string(),
                kind: "host_metrics".to_string(),
                target_ref: "host-1:host".to_string(),
                resource_ref: Some("host-1".to_string()),
                unit: "1".to_string(),
            },
            values: vec![0.25],
            timestamps: vec![1_234_567_890_000],
        };

        sink.write_metrics(&envelope, &metrics)
            .await
            .expect("write metrics");

        let body = server.await.expect("join");
        assert!(body.contains(" METRICS: "), "frame: {body}");
        assert!(body.contains("\"agent\":\"agent-001\""), "frame: {body}");
        assert!(
            body.contains("\"__name__\":\"system.load_average.1m\""),
            "frame: {body}"
        );
    }
}
