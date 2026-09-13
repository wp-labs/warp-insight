//! Agent 级全局 `seq` 高水位存储（`state/logs/seq.json`）。
//!
//! 与各 input 的 checkpoint 解耦：误删某个 input 的 checkpoint 不再回退全局号源。
//! 写入采用原子写（tmp + fsync + rename），崩溃不会产生半写文件。

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::fs_async::{read_json_async, write_json_atomic_async};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LogSeqState {
    /// 下一个待分配的 `seq`（per-`agent` 全局单调递增）。
    #[serde(default)]
    pub next_seq: u64,
}

impl LogSeqState {
    pub(crate) fn new(next_seq: u64) -> Self {
        Self { next_seq }
    }
}

pub(crate) fn path_for(state_dir: &Path) -> PathBuf {
    state_dir.join("logs").join("seq.json")
}

/// 读取全局高水位；文件不存在视为首次运行，返回 0。
pub(crate) async fn load_or_default_async(path: &Path) -> io::Result<u64> {
    match tokio::fs::metadata(path).await {
        Ok(_) => {
            let state: LogSeqState = read_json_async(path).await?;
            Ok(state.next_seq)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(err) => Err(err),
    }
}

/// 原子写入全局高水位。
pub(crate) async fn store_async(path: &Path, next_seq: u64) -> io::Result<()> {
    write_json_atomic_async(path, &LogSeqState::new(next_seq)).await
}
