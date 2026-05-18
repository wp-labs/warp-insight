//! Warp Parse ingress protocol contracts and fixed-width ASCII head codec.

use std::error::Error;
use std::fmt;
use std::str;

use serde::{Deserialize, Serialize};

use crate::API_VERSION_V1;
use crate::discovery::DiscoverySnapshotContract;

pub const REPORT_DISCOVERY_SNAPSHOT_KIND: &str = "report_discovery_snapshot";
pub const DISCOVERY_INGEST_ACK_KIND: &str = "discovery_ingest_ack";
pub const WARP_PARSE_INGEST_HEAD_MAGIC: &str = "WPI1";
pub const WARP_PARSE_INGEST_HEAD_LEN: usize = 64;
const BODY_LEN_WIDTH: usize = 9;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportDiscoverySnapshot {
    pub api_version: String,
    pub kind: String,
    pub report_id: String,
    pub agent_id: String,
    pub instance_id: String,
    pub snapshot_id: String,
    pub revision: u64,
    pub generated_at: String,
    pub report_attempt: u32,
    pub report_mode: DiscoveryReportMode,
    pub reported_at: String,
    pub snapshot: DiscoverySnapshotContract,
}

impl ReportDiscoverySnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        report_id: String,
        agent_id: String,
        instance_id: String,
        report_attempt: u32,
        report_mode: DiscoveryReportMode,
        reported_at: String,
        snapshot: DiscoverySnapshotContract,
    ) -> Self {
        Self {
            api_version: API_VERSION_V1.to_string(),
            kind: REPORT_DISCOVERY_SNAPSHOT_KIND.to_string(),
            report_id,
            agent_id,
            instance_id,
            snapshot_id: snapshot.snapshot_id.clone(),
            revision: snapshot.revision,
            generated_at: snapshot.generated_at.clone(),
            report_attempt,
            report_mode,
            reported_at,
            snapshot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoveryReportMode {
    #[serde(rename = "full_snapshot")]
    FullSnapshot,
    #[serde(rename = "snapshot_replace")]
    SnapshotReplace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryIngestAck {
    pub api_version: String,
    pub kind: String,
    pub report_id: String,
    pub agent_id: String,
    pub instance_id: String,
    pub snapshot_id: String,
    pub revision: u64,
    pub ack_status: DiscoveryIngestAckStatus,
    pub accepted_at: String,
    pub reason_code: Option<String>,
    pub reason_message: Option<String>,
}

impl DiscoveryIngestAck {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        report_id: String,
        agent_id: String,
        instance_id: String,
        snapshot_id: String,
        revision: u64,
        ack_status: DiscoveryIngestAckStatus,
        accepted_at: String,
    ) -> Self {
        Self {
            api_version: API_VERSION_V1.to_string(),
            kind: DISCOVERY_INGEST_ACK_KIND.to_string(),
            report_id,
            agent_id,
            instance_id,
            snapshot_id,
            revision,
            ack_status,
            accepted_at,
            reason_code: None,
            reason_message: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoveryIngestAckStatus {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "duplicate")]
    Duplicate,
    #[serde(rename = "stale")]
    Stale,
    #[serde(rename = "rejected")]
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarpParseIngestHead {
    pub version: u8,
    pub message_kind: WarpParseIngestMessageKind,
    pub encoding: WarpParseIngestEncoding,
    pub compression: WarpParseIngestCompression,
    pub body_len: u32,
    pub flags: u8,
}

impl WarpParseIngestHead {
    pub fn discovery_snapshot(body_len: u32) -> Self {
        Self {
            version: 1,
            message_kind: WarpParseIngestMessageKind::DiscoverySnapshot,
            encoding: WarpParseIngestEncoding::Json,
            compression: WarpParseIngestCompression::None,
            body_len,
            flags: 0,
        }
    }

    pub fn encode(self) -> Result<[u8; WARP_PARSE_INGEST_HEAD_LEN], IngestHeadError> {
        if self.version == 0 || self.version > 9 {
            return Err(IngestHeadError::InvalidVersion(self.version));
        }
        if self.body_len > 999_999_999 {
            return Err(IngestHeadError::BodyLenOutOfRange(self.body_len));
        }

        let head = format!(
            "{magic};V={version};K={kind};E={encoding};C={compression};L={body_len:0width$};F={flags:02X};",
            magic = WARP_PARSE_INGEST_HEAD_MAGIC,
            version = self.version,
            kind = self.message_kind.as_code(),
            encoding = self.encoding.as_code(),
            compression = self.compression.as_code(),
            body_len = self.body_len,
            width = BODY_LEN_WIDTH,
            flags = self.flags,
        );

        if !head.is_ascii() {
            return Err(IngestHeadError::NonAsciiHead);
        }
        if head.len() > WARP_PARSE_INGEST_HEAD_LEN {
            return Err(IngestHeadError::HeadTooLong(head.len()));
        }

        let mut buf = [b' '; WARP_PARSE_INGEST_HEAD_LEN];
        buf[..head.len()].copy_from_slice(head.as_bytes());
        Ok(buf)
    }

    pub fn decode(input: &[u8]) -> Result<Self, IngestHeadError> {
        if input.len() != WARP_PARSE_INGEST_HEAD_LEN {
            return Err(IngestHeadError::InvalidHeadLen(input.len()));
        }
        if !input.is_ascii() {
            return Err(IngestHeadError::NonAsciiHead);
        }

        let trimmed = str::from_utf8(input)
            .map_err(|_| IngestHeadError::NonAsciiHead)?
            .trim_end_matches(' ');
        let parts: Vec<&str> = trimmed.split(';').collect();
        if parts.len() != 8 || parts.last() != Some(&"") {
            return Err(IngestHeadError::InvalidFieldLayout);
        }

        if parts[0] != WARP_PARSE_INGEST_HEAD_MAGIC {
            return Err(IngestHeadError::InvalidMagic(parts[0].to_string()));
        }

        let version = parse_kv(parts[1], "V")?
            .parse::<u8>()
            .map_err(|_| IngestHeadError::InvalidVersionField(parts[1].to_string()))?;
        let message_kind = WarpParseIngestMessageKind::from_code(parse_kv(parts[2], "K")?)?;
        let encoding = WarpParseIngestEncoding::from_code(parse_kv(parts[3], "E")?)?;
        let compression = WarpParseIngestCompression::from_code(parse_kv(parts[4], "C")?)?;
        let body_len_raw = parse_kv(parts[5], "L")?;
        if body_len_raw.len() != BODY_LEN_WIDTH || !body_len_raw.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(IngestHeadError::InvalidBodyLenField(parts[5].to_string()));
        }
        let body_len = body_len_raw
            .parse::<u32>()
            .map_err(|_| IngestHeadError::InvalidBodyLenField(parts[5].to_string()))?;
        let flags_raw = parse_kv(parts[6], "F")?;
        if flags_raw.len() != 2 {
            return Err(IngestHeadError::InvalidFlagsField(parts[6].to_string()));
        }
        let flags = u8::from_str_radix(flags_raw, 16)
            .map_err(|_| IngestHeadError::InvalidFlagsField(parts[6].to_string()))?;

        Ok(Self {
            version,
            message_kind,
            encoding,
            compression,
            body_len,
            flags,
        })
    }
}

fn parse_kv<'a>(field: &'a str, key: &str) -> Result<&'a str, IngestHeadError> {
    let (parsed_key, value) = field
        .split_once('=')
        .ok_or_else(|| IngestHeadError::InvalidField(field.to_string()))?;
    if parsed_key != key {
        return Err(IngestHeadError::UnexpectedField {
            expected: key.to_string(),
            actual: parsed_key.to_string(),
        });
    }
    if value.is_empty() {
        return Err(IngestHeadError::InvalidField(field.to_string()));
    }
    Ok(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpParseIngestMessageKind {
    DiscoverySnapshot,
    DiscoveryIngestAck,
}

impl WarpParseIngestMessageKind {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::DiscoverySnapshot => "DSNAP",
            Self::DiscoveryIngestAck => "DACK",
        }
    }

    pub fn from_code(code: &str) -> Result<Self, IngestHeadError> {
        match code {
            "DSNAP" => Ok(Self::DiscoverySnapshot),
            "DACK" => Ok(Self::DiscoveryIngestAck),
            _ => Err(IngestHeadError::UnsupportedMessageKind(code.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpParseIngestEncoding {
    Json,
}

impl WarpParseIngestEncoding {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Json => "JSON",
        }
    }

    pub fn from_code(code: &str) -> Result<Self, IngestHeadError> {
        match code {
            "JSON" => Ok(Self::Json),
            _ => Err(IngestHeadError::UnsupportedEncoding(code.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpParseIngestCompression {
    None,
    Gzip,
    Zstd,
}

impl WarpParseIngestCompression {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Gzip => "GZIP",
            Self::Zstd => "ZSTD",
        }
    }

    pub fn from_code(code: &str) -> Result<Self, IngestHeadError> {
        match code {
            "NONE" => Ok(Self::None),
            "GZIP" => Ok(Self::Gzip),
            "ZSTD" => Ok(Self::Zstd),
            _ => Err(IngestHeadError::UnsupportedCompression(code.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestHeadError {
    InvalidHeadLen(usize),
    NonAsciiHead,
    InvalidMagic(String),
    InvalidFieldLayout,
    InvalidField(String),
    UnexpectedField { expected: String, actual: String },
    InvalidVersion(u8),
    InvalidVersionField(String),
    UnsupportedMessageKind(String),
    UnsupportedEncoding(String),
    UnsupportedCompression(String),
    InvalidBodyLenField(String),
    BodyLenOutOfRange(u32),
    InvalidFlagsField(String),
    HeadTooLong(usize),
}

impl fmt::Display for IngestHeadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeadLen(len) => write!(f, "invalid ingest head length: {len}"),
            Self::NonAsciiHead => write!(f, "ingest head must be ASCII"),
            Self::InvalidMagic(magic) => write!(f, "invalid ingest head magic: {magic}"),
            Self::InvalidFieldLayout => write!(f, "invalid ingest head field layout"),
            Self::InvalidField(field) => write!(f, "invalid ingest head field: {field}"),
            Self::UnexpectedField { expected, actual } => {
                write!(
                    f,
                    "unexpected ingest head field: expected {expected}, got {actual}"
                )
            }
            Self::InvalidVersion(version) => write!(f, "invalid ingest head version: {version}"),
            Self::InvalidVersionField(field) => {
                write!(f, "invalid ingest head version field: {field}")
            }
            Self::UnsupportedMessageKind(kind) => {
                write!(f, "unsupported ingest message kind: {kind}")
            }
            Self::UnsupportedEncoding(encoding) => {
                write!(f, "unsupported ingest encoding: {encoding}")
            }
            Self::UnsupportedCompression(compression) => {
                write!(f, "unsupported ingest compression: {compression}")
            }
            Self::InvalidBodyLenField(field) => {
                write!(f, "invalid ingest head body length field: {field}")
            }
            Self::BodyLenOutOfRange(body_len) => {
                write!(f, "ingest head body length out of range: {body_len}")
            }
            Self::InvalidFlagsField(field) => {
                write!(f, "invalid ingest head flags field: {field}")
            }
            Self::HeadTooLong(len) => write!(f, "encoded ingest head too long: {len}"),
        }
    }
}

impl Error for IngestHeadError {}

#[cfg(test)]
mod tests {
    use super::{
        DISCOVERY_INGEST_ACK_KIND, DiscoveryIngestAck, DiscoveryIngestAckStatus,
        DiscoveryReportMode, REPORT_DISCOVERY_SNAPSHOT_KIND, ReportDiscoverySnapshot,
        WARP_PARSE_INGEST_HEAD_LEN, WarpParseIngestCompression, WarpParseIngestEncoding,
        WarpParseIngestHead, WarpParseIngestMessageKind,
    };
    use crate::API_VERSION_V1;
    use std::collections::BTreeMap;

    use crate::discovery::{DiscoveredResource, DiscoveryOrigin, DiscoverySnapshotContract};

    fn sample_snapshot() -> DiscoverySnapshotContract {
        let mut snapshot = DiscoverySnapshotContract::new(
            "discovery:1:2026-04-20T00:00:00Z".to_string(),
            1,
            "2026-04-20T00:00:00Z".to_string(),
        );
        snapshot.origins.push(DiscoveryOrigin {
            origin_id: "origin-01".to_string(),
            probe: "host".to_string(),
            source: "local_runtime".to_string(),
            observed_at: "2026-04-20T00:00:00Z".to_string(),
        });
        snapshot.resources.push(DiscoveredResource {
            resource_id: "host:host-01".to_string(),
            kind: "host".to_string(),
            origin_idx: 0,
            attributes: BTreeMap::from([("host.id".to_string(), "host-01".to_string())]),
            discovered_at: "2026-04-20T00:00:00Z".to_string(),
            last_seen_at: "2026-04-20T00:00:00Z".to_string(),
            health: "healthy".to_string(),
            source: "local_runtime".to_string(),
        });
        snapshot
    }

    #[test]
    fn report_discovery_snapshot_new_sets_contract_fields() {
        let snapshot = sample_snapshot();
        let report = ReportDiscoverySnapshot::new(
            "disrep_01".to_string(),
            "agent-01".to_string(),
            "inst-01".to_string(),
            1,
            DiscoveryReportMode::FullSnapshot,
            "2026-04-20T00:00:02Z".to_string(),
            snapshot.clone(),
        );

        assert_eq!(report.api_version, API_VERSION_V1);
        assert_eq!(report.kind, REPORT_DISCOVERY_SNAPSHOT_KIND);
        assert_eq!(report.snapshot_id, snapshot.snapshot_id);
        assert_eq!(report.revision, snapshot.revision);
        assert_eq!(report.generated_at, snapshot.generated_at);
        assert_eq!(report.snapshot, snapshot);
    }

    #[test]
    fn discovery_ingest_ack_new_sets_contract_fields() {
        let ack = DiscoveryIngestAck::new(
            "disrep_01".to_string(),
            "agent-01".to_string(),
            "inst-01".to_string(),
            "discovery:1:2026-04-20T00:00:00Z".to_string(),
            1,
            DiscoveryIngestAckStatus::Accepted,
            "2026-04-20T00:00:03Z".to_string(),
        );

        assert_eq!(ack.api_version, API_VERSION_V1);
        assert_eq!(ack.kind, DISCOVERY_INGEST_ACK_KIND);
        assert_eq!(ack.ack_status, DiscoveryIngestAckStatus::Accepted);
    }

    #[test]
    fn discovery_report_round_trips_with_serde_json() {
        let report = ReportDiscoverySnapshot::new(
            "disrep_01".to_string(),
            "agent-01".to_string(),
            "inst-01".to_string(),
            1,
            DiscoveryReportMode::FullSnapshot,
            "2026-04-20T00:00:02Z".to_string(),
            sample_snapshot(),
        );

        let json = serde_json::to_string(&report).expect("serialize report");
        let decoded: ReportDiscoverySnapshot =
            serde_json::from_str(&json).expect("deserialize report");

        assert_eq!(decoded, report);
    }

    #[test]
    fn discovery_ingest_ack_round_trips_with_serde_json() {
        let ack = DiscoveryIngestAck::new(
            "disrep_01".to_string(),
            "agent-01".to_string(),
            "inst-01".to_string(),
            "discovery:1:2026-04-20T00:00:00Z".to_string(),
            1,
            DiscoveryIngestAckStatus::Duplicate,
            "2026-04-20T00:00:03Z".to_string(),
        );

        let json = serde_json::to_string(&ack).expect("serialize ack");
        let decoded: DiscoveryIngestAck = serde_json::from_str(&json).expect("deserialize ack");

        assert_eq!(decoded, ack);
    }

    #[test]
    fn ingest_head_encodes_to_fixed_width_ascii() {
        let encoded = WarpParseIngestHead::discovery_snapshot(18_432)
            .encode()
            .expect("encode head");
        let head = std::str::from_utf8(&encoded).expect("utf8 head");

        assert_eq!(encoded.len(), WARP_PARSE_INGEST_HEAD_LEN);
        assert!(head.starts_with("WPI1;V=1;K=DSNAP;E=JSON;C=NONE;L=000018432;F=00;"));
        assert!(encoded.is_ascii());
    }

    #[test]
    fn ingest_head_round_trips() {
        let original = WarpParseIngestHead {
            version: 1,
            message_kind: WarpParseIngestMessageKind::DiscoverySnapshot,
            encoding: WarpParseIngestEncoding::Json,
            compression: WarpParseIngestCompression::None,
            body_len: 12_345,
            flags: 0,
        };

        let encoded = original.encode().expect("encode head");
        let decoded = WarpParseIngestHead::decode(&encoded).expect("decode head");

        assert_eq!(decoded, original);
    }

    #[test]
    fn ingest_head_decode_rejects_wrong_field_order() {
        let mut head = [b' '; WARP_PARSE_INGEST_HEAD_LEN];
        let raw = b"WPI1;K=DSNAP;V=1;E=JSON;C=NONE;L=000000001;F=00;";
        head[..raw.len()].copy_from_slice(raw);

        let err = WarpParseIngestHead::decode(&head).expect_err("reject wrong order");
        assert!(err.to_string().contains("unexpected ingest head field"));
    }

    #[test]
    fn ingest_head_decode_rejects_unknown_message_kind() {
        let mut head = [b' '; WARP_PARSE_INGEST_HEAD_LEN];
        let raw = b"WPI1;V=1;K=OTHER;E=JSON;C=NONE;L=000000001;F=00;";
        head[..raw.len()].copy_from_slice(raw);

        let err = WarpParseIngestHead::decode(&head).expect_err("reject unknown kind");
        assert!(err.to_string().contains("unsupported ingest message kind"));
    }
}
