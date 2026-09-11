//! Reading complete lines from a tracked log file.

use std::fs;
use std::io::{self, SeekFrom};
use std::path::{Path, PathBuf};

use tokio::fs::File;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, BufReader};

pub const CHECKPOINT_PROBE_BYTES: usize = 16;

/// 单次读取限制：长行保护 + 大文件回放分块（见 log-file-input-spec §7.3/§12）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadLimits {
    /// 单行最大字节数：超过即截断提交，并计入 `truncated_lines`。
    pub max_line_bytes: usize,
    /// 单次最多消费的字节数（在行边界处停止，保证下次从行首继续）。
    pub max_bytes: usize,
    /// 单次最多提交的行数。
    pub max_lines: usize,
}

impl ReadLimits {
    pub fn new(max_line_bytes: usize, max_bytes: usize, max_lines: usize) -> Self {
        Self {
            max_line_bytes: max_line_bytes.max(1),
            max_bytes: max_bytes.max(1),
            max_lines: max_lines.max(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Observed", module = "Observed.Entity")]
pub struct ObservedFileIdentity {
    pub device_id: Option<u64>,
    pub inode: Option<u64>,
    pub fingerprint: Option<String>,
    pub file_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct RawFileLine {
    pub text: String,
    pub start_offset: u64,
    pub end_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct ReadFromOffset {
    pub identity: ObservedFileIdentity,
    pub lines: Vec<RawFileLine>,
    pub committed_end_offset: u64,
    /// 本次读取中被截断的超长行数（每条截断行已作为一条记录提交）。
    pub truncated_lines: usize,
}

pub async fn stable_file_id_async(path: &Path, identity: &ObservedFileIdentity) -> String {
    match (identity.device_id, identity.inode) {
        (Some(device_id), Some(inode)) => format!("dev:{device_id}:ino:{inode}"),
        _ => {
            let canonical = canonical_key_path_async(path).await;
            identity
                .fingerprint
                .as_ref()
                .map(|fingerprint| {
                    format!("path:{}:fingerprint:{fingerprint}", canonical.display())
                })
                .unwrap_or_else(|| format!("path:{}", canonical.display()))
        }
    }
}

async fn canonical_key_path_async(path: &Path) -> PathBuf {
    tokio::fs::canonicalize(path)
        .await
        .unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
pub fn stable_file_id(path: &Path, identity: &ObservedFileIdentity) -> String {
    block_on(stable_file_id_async(path, identity))
}

pub async fn inspect_path_async(path: &Path) -> io::Result<ObservedFileIdentity> {
    let mut file = File::open(path).await?;
    let metadata = file.metadata().await?;
    let prefix = read_prefix_async(&mut file).await?;
    Ok(identity_from_metadata(&metadata, prefix))
}

#[cfg(test)]
pub fn inspect_path(path: &Path) -> io::Result<ObservedFileIdentity> {
    block_on(inspect_path_async(path))
}

/// 从 `start_offset` 起读取完整行。
///
/// - 只提交完整行（文件尾部的半行不提交，下次继续）；
/// - 单行超过 `max_line_bytes`：截断提交（内容保留前 `max_line_bytes`），
///   跳过该行剩余到行尾后原子提交，并计入 `truncated_lines`；
/// - 达到 `max_lines` / `max_bytes` 时在行边界停止，保证下次从行首继续；
///   行内不因预算中断——单行可能超过 `max_bytes`，但不会被切成两半。
pub async fn read_from_offset_async(
    path: &Path,
    start_offset: u64,
    limits: ReadLimits,
) -> io::Result<ReadFromOffset> {
    let mut file = File::open(path).await?;
    let metadata = file.metadata().await?;
    let prefix = read_prefix_async(&mut file).await?;
    let identity = identity_from_metadata(&metadata, prefix);
    let bounded_offset = start_offset.min(identity.file_len);
    file.seek(SeekFrom::Start(bounded_offset)).await?;

    let mut reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut truncated_lines = 0usize;
    let mut committed_end_offset = bounded_offset;
    let mut line_start = bounded_offset;
    let mut line_consumed: u64 = 0;
    let mut consumed_total: u64 = 0;
    let mut line_buf: Vec<u8> = Vec::new();

    loop {
        if lines.len() >= limits.max_lines {
            break;
        }
        // 字节预算只在行边界结算：行内不因预算中断，否则长行（长度 > 预算）
        // 会每轮从行首重读、永不推进。
        if line_consumed == 0 && consumed_total >= limits.max_bytes as u64 {
            break;
        }

        let Some(chunk) = read_chunk_async(&mut reader).await? else {
            break;
        };

        // 行边界处若下一行会越过 `max_bytes`，停在当前行首，下次从行首继续。
        if line_consumed == 0
            && consumed_total > 0
            && consumed_total + chunk.take as u64 > limits.max_bytes as u64
        {
            break;
        }

        let truncated = append_bounded(&mut line_buf, &chunk.bytes, limits.max_line_bytes);
        reader.consume(chunk.take);
        line_consumed += chunk.take as u64;
        consumed_total += chunk.take as u64;

        if !chunk.has_newline {
            if !truncated {
                continue;
            }
            // 超长行：跳过剩余字节到行尾，再原子提交截断行。
            let skipped = skip_to_line_end_async(&mut reader).await?;
            line_consumed += skipped;
            consumed_total += skipped;
        }

        let end = line_start + line_consumed;
        lines.push(RawFileLine {
            text: String::from_utf8_lossy(&line_buf).to_string(),
            start_offset: line_start,
            end_offset: end,
        });
        if truncated {
            truncated_lines += 1;
        }
        committed_end_offset = end;
        line_start = end;
        line_consumed = 0;
        line_buf.clear();
    }

    Ok(ReadFromOffset {
        identity,
        lines,
        committed_end_offset,
        truncated_lines,
    })
}

#[cfg(test)]
pub fn read_from_offset(
    path: &Path,
    start_offset: u64,
    limits: ReadLimits,
) -> io::Result<ReadFromOffset> {
    block_on(read_from_offset_async(path, start_offset, limits))
}

struct ReadChunk {
    take: usize,
    has_newline: bool,
    bytes: Vec<u8>,
}

/// 读取至行尾或缓冲区末尾；`None` 表示已到文件末尾。
async fn read_chunk_async(
    reader: &mut (impl AsyncBufRead + Unpin),
) -> io::Result<Option<ReadChunk>> {
    let available = reader.fill_buf().await?;
    if available.is_empty() {
        return Ok(None);
    }
    if let Some(pos) = available.iter().position(|byte| *byte == b'\n') {
        let take = pos + 1;
        Ok(Some(ReadChunk {
            take,
            has_newline: true,
            bytes: available[..take].to_vec(),
        }))
    } else {
        let take = available.len();
        Ok(Some(ReadChunk {
            take,
            has_newline: false,
            bytes: available.to_vec(),
        }))
    }
}

/// 跳过当前行剩余字节到行尾（或文件末尾），返回跳过的字节数。
async fn skip_to_line_end_async(reader: &mut (impl AsyncBufRead + Unpin)) -> io::Result<u64> {
    let mut skipped = 0u64;
    loop {
        let (take, found_newline) = {
            let rest = reader.fill_buf().await?;
            if rest.is_empty() {
                return Ok(skipped);
            }
            match rest.iter().position(|byte| *byte == b'\n') {
                Some(pos) => (pos + 1, true),
                None => (rest.len(), false),
            }
        };
        reader.consume(take);
        skipped += take as u64;
        if found_newline {
            return Ok(skipped);
        }
    }
}

/// 追加字节但不超过 `max`；返回是否发生了截断。
fn append_bounded(buf: &mut Vec<u8>, chunk: &[u8], max: usize) -> bool {
    if buf.len() >= max {
        return true;
    }
    let room = max - buf.len();
    if chunk.len() > room {
        buf.extend_from_slice(&chunk[..room]);
        true
    } else {
        buf.extend_from_slice(chunk);
        false
    }
}

pub async fn checkpoint_probe_async(
    path: &Path,
    checkpoint_offset: u64,
) -> io::Result<Option<String>> {
    if checkpoint_offset == 0 {
        return Ok(None);
    }

    let mut file = File::open(path).await?;
    let probe_len = (checkpoint_offset as usize).min(CHECKPOINT_PROBE_BYTES);
    let probe_start = checkpoint_offset - probe_len as u64;
    file.seek(SeekFrom::Start(probe_start)).await?;
    let mut buf = vec![0u8; probe_len];
    file.read_exact(&mut buf).await?;
    Ok(Some(fingerprint(&buf)))
}

#[cfg(test)]
pub fn checkpoint_probe(path: &Path, checkpoint_offset: u64) -> io::Result<Option<String>> {
    block_on(checkpoint_probe_async(path, checkpoint_offset))
}

fn identity_from_metadata(metadata: &fs::Metadata, prefix: Vec<u8>) -> ObservedFileIdentity {
    ObservedFileIdentity {
        device_id: device_id(metadata),
        inode: inode(metadata),
        fingerprint: Some(fingerprint(&prefix)),
        file_len: metadata.len(),
    }
}

async fn read_prefix_async(file: &mut File) -> io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0)).await?;
    let mut prefix = vec![0u8; 32];
    let size = file.read(&mut prefix).await?;
    prefix.truncate(size);
    Ok(prefix)
}

#[cfg(unix)]
fn device_id(metadata: &fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.dev())
}

#[cfg(not(unix))]
fn device_id(_metadata: &fs::Metadata) -> Option<u64> {
    None
}

#[cfg(unix)]
fn inode(metadata: &fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.ino())
}

#[cfg(not(unix))]
fn inode(_metadata: &fs::Metadata) -> Option<u64> {
    None
}

fn fingerprint(bytes: &[u8]) -> String {
    let mut out = String::new();
    for byte in bytes {
        out.push(nibble_to_hex(byte >> 4));
        out.push(nibble_to_hex(byte & 0x0f));
    }
    out
}

fn nibble_to_hex(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        _ => (b'a' + (value - 10)) as char,
    }
}

#[cfg(test)]
fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
        .block_on(future)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        ObservedFileIdentity, ReadLimits, checkpoint_probe, inspect_path, read_from_offset,
        stable_file_id,
    };

    fn temp_file(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        std::env::temp_dir().join(format!("warp-agentd-file-reader-{name}-{suffix}.log"))
    }

    fn default_limits() -> ReadLimits {
        ReadLimits::new(1_048_576, 4_194_304, 4096)
    }

    #[test]
    fn reads_only_complete_lines_and_keeps_partial_tail_uncommitted() {
        let path = temp_file("complete-lines");
        fs::write(&path, "first\nsecond\nthird").expect("write file");

        let read = read_from_offset(&path, 0, default_limits()).expect("read");

        assert_eq!(read.lines.len(), 2);
        assert_eq!(read.lines[0].text, "first\n");
        assert_eq!(read.lines[1].text, "second\n");
        assert_eq!(read.committed_end_offset, "first\nsecond\n".len() as u64);
        assert_eq!(read.truncated_lines, 0);
        fs::remove_file(path).ok();
    }

    #[test]
    fn inspect_path_reads_only_prefix_for_fingerprint() {
        let path = temp_file("inspect");
        fs::write(&path, "abcdefghijklmnopqrstuvwxyz1234567890").expect("write file");

        let identity = inspect_path(&path).expect("inspect");

        assert_eq!(identity.file_len, 36);
        assert_eq!(
            identity.fingerprint.as_deref(),
            Some("6162636465666768696a6b6c6d6e6f707172737475767778797a313233343536")
        );
        fs::remove_file(path).ok();
    }

    #[test]
    fn stable_file_id_ignores_path_when_device_and_inode_are_available() {
        let identity = ObservedFileIdentity {
            device_id: Some(11),
            inode: Some(22),
            fingerprint: Some("616263".to_string()),
            file_len: 3,
        };

        let first = stable_file_id(PathBuf::from("/tmp/app.log").as_path(), &identity);
        let second = stable_file_id(PathBuf::from("/tmp/app.log.1").as_path(), &identity);

        assert_eq!(first, second);
        assert_eq!(first, "dev:11:ino:22");
    }

    #[test]
    fn stable_file_id_includes_path_when_device_and_inode_are_unavailable() {
        let identity = ObservedFileIdentity {
            device_id: None,
            inode: None,
            fingerprint: Some("616263".to_string()),
            file_len: 3,
        };

        let first = stable_file_id(Path::new("/tmp/app.log"), &identity);
        let second = stable_file_id(Path::new("/tmp/other.log"), &identity);

        assert_ne!(first, second);
        assert!(first.contains("path:"));
        assert!(first.contains("fingerprint:616263"));
    }

    #[test]
    fn checkpoint_probe_uses_trailing_bytes_before_offset() {
        let path = temp_file("probe");
        fs::write(&path, "first\nsecond\nthird\n").expect("write file");

        let probe = checkpoint_probe(&path, "first\nsecond\n".len() as u64).expect("probe");

        assert_eq!(probe.as_deref(), Some("66697273740a7365636f6e640a"));
        fs::remove_file(path).ok();
    }

    #[test]
    fn truncates_over_long_line_and_counts_it() {
        let path = temp_file("long-line");
        fs::write(&path, "a".repeat(3000)).expect("write file");
        let limits = ReadLimits::new(1024, 1_048_576, 64);

        let read = read_from_offset(&path, 0, limits).expect("read");

        assert_eq!(read.lines.len(), 1);
        assert_eq!(read.truncated_lines, 1);
        assert_eq!(read.lines[0].text.len(), 1024);
        assert_eq!(read.lines[0].start_offset, 0);
        // 截断行的 end_offset 指向真实行尾（文件尾），checkpoint 不会卡住。
        assert_eq!(read.lines[0].end_offset, 3000);
        assert_eq!(read.committed_end_offset, 3000);
        fs::remove_file(path).ok();
    }

    #[test]
    fn long_line_truncation_keeps_following_line() {
        let path = temp_file("long-line-then-normal");
        let mut content = "b".repeat(2048);
        content.push('\n');
        content.push_str("next\n");
        fs::write(&path, &content).expect("write file");
        let limits = ReadLimits::new(1024, 1_048_576, 64);

        let read = read_from_offset(&path, 0, limits).expect("read");

        assert_eq!(read.lines.len(), 2);
        assert_eq!(read.truncated_lines, 1);
        assert_eq!(read.lines[0].text.len(), 1024);
        assert_eq!(read.lines[0].end_offset, 2049);
        assert_eq!(read.lines[1].text, "next\n");
        assert_eq!(read.lines[1].start_offset, 2049);
        assert_eq!(read.committed_end_offset, content.len() as u64);
        fs::remove_file(path).ok();
    }

    #[test]
    fn chunks_reads_by_max_lines_and_resumes_at_line_boundary() {
        let path = temp_file("chunked-lines");
        fs::write(&path, "l1\nl2\nl3\nl4\nl5\n").expect("write file");
        let limits = ReadLimits::new(1024, 1_048_576, 2);

        let first = read_from_offset(&path, 0, limits).expect("first read");
        assert_eq!(first.lines.len(), 2);
        assert_eq!(first.committed_end_offset, "l1\nl2\n".len() as u64);

        let second = read_from_offset(&path, first.committed_end_offset, limits).expect("second");
        assert_eq!(second.lines.len(), 2);
        assert_eq!(second.lines[0].text, "l3\n");
        assert_eq!(second.committed_end_offset, "l1\nl2\nl3\nl4\n".len() as u64);

        let third = read_from_offset(&path, second.committed_end_offset, limits).expect("third");
        assert_eq!(third.lines.len(), 1);
        assert_eq!(third.lines[0].text, "l5\n");
        fs::remove_file(path).ok();
    }

    #[test]
    fn stops_at_line_boundary_when_max_bytes_is_reached() {
        let path = temp_file("chunked-bytes");
        fs::write(&path, "aaaa\nbbbb\ncccc\n").expect("write file");
        // 每行 5 字节；上限 6 字节 → 一轮最多提交 1 行（第二行会越过上限）。
        let limits = ReadLimits::new(1024, 6, 64);

        let read = read_from_offset(&path, 0, limits).expect("read");

        assert_eq!(read.lines.len(), 1);
        assert_eq!(read.lines[0].text, "aaaa\n");
        assert_eq!(read.committed_end_offset, 5);
        fs::remove_file(path).ok();
    }

    #[test]
    fn resumes_from_line_start_after_byte_budget_stop() {
        let path = temp_file("byte-budget-resume");
        fs::write(&path, "aaaa\nbbbb\n").expect("write file");
        let limits = ReadLimits::new(1024, 6, 64);

        let first = read_from_offset(&path, 0, limits).expect("first read");
        assert_eq!(first.lines.len(), 1);
        assert_eq!(first.committed_end_offset, 5);

        let second = read_from_offset(&path, first.committed_end_offset, limits).expect("second");
        assert_eq!(second.lines.len(), 1);
        assert_eq!(second.lines[0].text, "bbbb\n");
        assert_eq!(second.lines[0].start_offset, 5);
        assert_eq!(second.committed_end_offset, 10);
        fs::remove_file(path).ok();
    }

    // Review 1：行长 > 预算但 < 单行上限，且跨多个缓冲块——必须能推进，不能卡死。
    #[test]
    fn long_line_larger_than_read_budget_still_completes() {
        let path = temp_file("line-over-budget");
        let mut content = "a".repeat(20_000);
        content.push('\n');
        content.push_str("tail\n");
        fs::write(&path, &content).expect("write file");
        // 预算 16 字节 << 行长 20001 字节，但单行上限 64KiB 不截断。
        let limits = ReadLimits::new(65_536, 16, 64);

        let read = read_from_offset(&path, 0, limits).expect("read");

        assert_eq!(read.lines.len(), 1);
        assert_eq!(read.lines[0].text.len(), 20_001);
        assert_eq!(read.truncated_lines, 0);
        assert_eq!(read.committed_end_offset, 20_001);

        let next = read_from_offset(&path, read.committed_end_offset, limits).expect("next");
        assert_eq!(next.lines.len(), 1);
        assert_eq!(next.lines[0].text, "tail\n");
        fs::remove_file(path).ok();
    }

    // Review 3：恰好等于上限不截断；超出 1 字节（含被丢弃的换行）即截断并计数。
    #[test]
    fn line_exactly_at_max_line_bytes_is_not_truncated() {
        let path = temp_file("line-at-limit");
        fs::write(&path, "abc\n").expect("write file");
        let limits = ReadLimits::new(4, 1_048_576, 64);

        let read = read_from_offset(&path, 0, limits).expect("read");

        assert_eq!(read.lines.len(), 1);
        assert_eq!(read.lines[0].text, "abc\n");
        assert_eq!(read.truncated_lines, 0);
        assert_eq!(read.committed_end_offset, 4);
        fs::remove_file(path).ok();
    }

    #[test]
    fn line_one_byte_over_max_line_bytes_is_truncated_and_counted() {
        let path = temp_file("line-over-limit-by-one");
        fs::write(&path, "abc\n").expect("write file");
        let limits = ReadLimits::new(3, 1_048_576, 64);

        let read = read_from_offset(&path, 0, limits).expect("read");

        assert_eq!(read.lines.len(), 1);
        assert_eq!(read.lines[0].text, "abc");
        assert_eq!(read.truncated_lines, 1);
        assert_eq!(read.committed_end_offset, 4);
        fs::remove_file(path).ok();
    }

    #[test]
    fn truncated_line_without_trailing_newline_is_committed_at_eof() {
        let path = temp_file("truncated-eof");
        fs::write(&path, "a".repeat(2000)).expect("write file");
        let limits = ReadLimits::new(1024, 1_048_576, 64);

        let read = read_from_offset(&path, 0, limits).expect("read");

        assert_eq!(read.lines.len(), 1);
        assert_eq!(read.truncated_lines, 1);
        assert_eq!(read.lines[0].text.len(), 1024);
        assert_eq!(read.committed_end_offset, 2000);
        fs::remove_file(path).ok();
    }

    #[test]
    fn multiple_truncated_lines_are_each_counted() {
        let path = temp_file("multiple-truncated");
        let mut content = "x".repeat(1500);
        content.push('\n');
        content.push_str("ok\n");
        content.push_str(&"y".repeat(1500));
        content.push('\n');
        fs::write(&path, &content).expect("write file");
        let limits = ReadLimits::new(1024, 1_048_576, 64);

        let read = read_from_offset(&path, 0, limits).expect("read");

        assert_eq!(read.lines.len(), 3);
        assert_eq!(read.truncated_lines, 2);
        assert_eq!(read.lines[1].text, "ok\n");
        assert_eq!(read.committed_end_offset, content.len() as u64);
        fs::remove_file(path).ok();
    }

    #[test]
    fn truncated_long_line_with_small_budget_still_completes() {
        let path = temp_file("truncated-over-budget");
        let mut content = "a".repeat(20_000);
        content.push('\n');
        content.push_str("tail\n");
        fs::write(&path, &content).expect("write file");
        // 预算 16 字节 + 单行上限 1024：分块截断必须在同一轮内跳完行尾并提交。
        let limits = ReadLimits::new(1024, 16, 64);

        let read = read_from_offset(&path, 0, limits).expect("read");

        assert_eq!(read.lines.len(), 1);
        assert_eq!(read.truncated_lines, 1);
        assert_eq!(read.lines[0].text.len(), 1024);
        assert_eq!(read.lines[0].end_offset, 20_001);
        assert_eq!(read.committed_end_offset, 20_001);

        let next = read_from_offset(&path, read.committed_end_offset, limits).expect("next");
        assert_eq!(next.lines.len(), 1);
        assert_eq!(next.lines[0].text, "tail\n");
        fs::remove_file(path).ok();
    }

    #[test]
    fn read_limits_clamp_zero_to_one_and_still_make_progress() {
        let limits = ReadLimits::new(0, 0, 0);
        assert_eq!(limits.max_line_bytes, 1);
        assert_eq!(limits.max_bytes, 1);
        assert_eq!(limits.max_lines, 1);

        let path = temp_file("zero-limits");
        fs::write(&path, "ab\nc\n").expect("write file");

        let read = read_from_offset(&path, 0, limits).expect("read");

        assert_eq!(read.lines.len(), 1);
        assert_eq!(read.truncated_lines, 1);
        assert_eq!(read.committed_end_offset, 3);
        fs::remove_file(path).ok();
    }
}
