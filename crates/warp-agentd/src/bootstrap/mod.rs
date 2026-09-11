//! Bootstrap entrypoints for `warp-agentd`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use wist_shared::paths::ACTIONS_DIR;

pub fn initialize(
    root_dir: &Path,
    run_dir: &Path,
    state_dir: &Path,
    log_dir: &Path,
) -> io::Result<()> {
    ensure_dirs([
        root_dir.to_path_buf(),
        run_dir.to_path_buf(),
        run_dir.join(ACTIONS_DIR),
        state_dir.to_path_buf(),
        log_dir.to_path_buf(),
    ])?;
    ensure_dirs(state_layout_dirs(state_dir))
}

pub async fn initialize_async(
    root_dir: &Path,
    run_dir: &Path,
    state_dir: &Path,
    log_dir: &Path,
) -> io::Result<()> {
    ensure_dirs_async([
        root_dir.to_path_buf(),
        run_dir.to_path_buf(),
        run_dir.join(ACTIONS_DIR),
        state_dir.to_path_buf(),
        log_dir.to_path_buf(),
    ])
    .await?;
    ensure_dirs_async(state_layout_dirs(state_dir)).await
}

/// Directories kept under `state_dir` by the runtime (persisted execution state).
fn state_layout_dirs(state_dir: &Path) -> [PathBuf; 4] {
    [
        state_dir.join("running"),
        state_dir.join("reporting"),
        state_dir.join("history"),
        state_dir.join("logs").join("file_inputs"),
    ]
}

/// Create every directory in `dirs`, stopping at the first failure.
fn ensure_dirs(dirs: impl IntoIterator<Item = PathBuf>) -> io::Result<()> {
    for dir in dirs {
        fs::create_dir_all(dir)?;
    }
    Ok(())
}

async fn ensure_dirs_async(dirs: impl IntoIterator<Item = PathBuf>) -> io::Result<()> {
    for dir in dirs {
        tokio::fs::create_dir_all(dir).await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
