//! Reading complete lines from a tracked log file.

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const CHECKPOINT_PROBE_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Observed", module = "Observed.Entity")]
pub struct ObservedFileIdentity {
    pub device_id: Option<u64>,
    pub inode: Option<u64>,
    pub fingerprint: Option<String>,
    pub file_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct RawFileLine {
    pub text: String,
    pub start_offset: u64,
    pub end_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Discovery", module = "Discovery.Collect")]
pub struct ReadFromOffset {
    pub identity: ObservedFileIdentity,
    pub lines: Vec<RawFileLine>,
    pub committed_end_offset: u64,
}

pub fn stable_file_id(path: &Path, identity: &ObservedFileIdentity) -> String {
    match (identity.device_id, identity.inode) {
        (Some(device_id), Some(inode)) => format!("dev:{device_id}:ino:{inode}"),
        _ => {
            let canonical = canonical_key_path(path);
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

fn canonical_key_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

pub fn inspect_path(path: &Path) -> io::Result<ObservedFileIdentity> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let prefix = read_prefix(&mut file)?;
    Ok(identity_from_metadata(&metadata, prefix))
}

pub fn read_from_offset(path: &Path, start_offset: u64) -> io::Result<ReadFromOffset> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let prefix = read_prefix(&mut file)?;
    let identity = identity_from_metadata(&metadata, prefix);
    let bounded_offset = start_offset.min(identity.file_len);
    file.seek(SeekFrom::Start(bounded_offset))?;

    let mut reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut committed_end_offset = bounded_offset;
    let mut line_start = bounded_offset;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let read = reader.read_until(b'\n', &mut buf)?;
        if read == 0 {
            break;
        }
        let end = line_start + read as u64;
        if buf.last() != Some(&b'\n') {
            break;
        }
        let text = String::from_utf8_lossy(&buf).to_string();
        lines.push(RawFileLine {
            text,
            start_offset: line_start,
            end_offset: end,
        });
        committed_end_offset = end;
        line_start = end;
    }

    Ok(ReadFromOffset {
        identity,
        lines,
        committed_end_offset,
    })
}

pub fn checkpoint_probe(path: &Path, checkpoint_offset: u64) -> io::Result<Option<String>> {
    if checkpoint_offset == 0 {
        return Ok(None);
    }

    let mut file = File::open(path)?;
    let probe_len = (checkpoint_offset as usize).min(CHECKPOINT_PROBE_BYTES);
    let probe_start = checkpoint_offset - probe_len as u64;
    file.seek(SeekFrom::Start(probe_start))?;
    let mut buf = vec![0u8; probe_len];
    file.read_exact(&mut buf)?;
    Ok(Some(fingerprint(&buf)))
}

fn identity_from_metadata(metadata: &fs::Metadata, prefix: Vec<u8>) -> ObservedFileIdentity {
    ObservedFileIdentity {
        device_id: device_id(metadata),
        inode: inode(metadata),
        fingerprint: Some(fingerprint(&prefix)),
        file_len: metadata.len(),
    }
}

fn read_prefix(file: &mut File) -> io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0))?;
    let mut prefix = vec![0u8; 32];
    let size = file.read(&mut prefix)?;
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
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        ObservedFileIdentity, checkpoint_probe, inspect_path, read_from_offset, stable_file_id,
    };

    fn temp_file(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        std::env::temp_dir().join(format!("warp-insightd-file-reader-{name}-{suffix}.log"))
    }

    #[test]
    fn reads_only_complete_lines_and_keeps_partial_tail_uncommitted() {
        let path = temp_file("complete-lines");
        fs::write(&path, "first\nsecond\nthird").expect("write file");

        let read = read_from_offset(&path, 0).expect("read");

        assert_eq!(read.lines.len(), 2);
        assert_eq!(read.lines[0].text, "first\n");
        assert_eq!(read.lines[1].text, "second\n");
        assert_eq!(read.committed_end_offset, "first\nsecond\n".len() as u64);
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
}
