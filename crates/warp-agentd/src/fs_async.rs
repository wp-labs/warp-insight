//! Async filesystem helpers (tokio-based) local to `warp-agentd`.

use std::io;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::AsyncWriteExt;

pub(crate) async fn ensure_parent_async(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    Ok(())
}

pub(crate) async fn read_json_async<T>(path: &Path) -> io::Result<T>
where
    T: DeserializeOwned,
{
    let text = tokio::fs::read_to_string(path).await?;
    serde_json::from_str(&text).map_err(io::Error::other)
}

pub(crate) async fn write_json_atomic_async<T>(path: &Path, value: &T) -> io::Result<()>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    write_bytes_atomic_async(path, &bytes).await
}

pub(crate) async fn write_json_private_atomic_async<T>(path: &Path, value: &T) -> io::Result<()>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    write_bytes_private_atomic_async(path, &bytes).await
}

pub(crate) async fn write_bytes_atomic_async(path: &Path, bytes: &[u8]) -> io::Result<()> {
    ensure_parent_async(path).await?;

    let tmp_path = path.with_extension("tmp");
    {
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        file.write_all(bytes).await?;
        file.write_all(b"\n").await?;
        file.sync_all().await?;
    }

    tokio::fs::rename(&tmp_path, path).await?;
    sync_parent_dir_async(path).await?;
    Ok(())
}

#[cfg(unix)]
pub(crate) async fn write_bytes_private_atomic_async(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    ensure_parent_async(path).await?;
    if let Some(parent) = path.parent() {
        tokio::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).await?;
    }

    let tmp_path = path.with_extension("tmp");
    {
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        file.write_all(bytes).await?;
        file.write_all(b"\n").await?;
        file.sync_all().await?;
    }

    tokio::fs::rename(&tmp_path, path).await?;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await?;
    sync_parent_dir_async(path).await?;
    Ok(())
}

#[cfg(not(unix))]
pub(crate) async fn write_bytes_private_atomic_async(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_bytes_atomic_async(path, bytes).await
}

#[cfg(unix)]
async fn sync_parent_dir_async(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::File::open(parent).await?.sync_all().await?;
    }
    Ok(())
}

#[cfg(not(unix))]
async fn sync_parent_dir_async(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_suffix() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("duration")
            .as_nanos()
    }

    #[tokio::test]
    async fn write_and_read_json_round_trip() {
        let path = std::env::temp_dir().join(format!("warp-agentd-fs-async-{}", unique_suffix()));
        let value = serde_json::json!({"k": "v", "n": 42});

        write_json_atomic_async(&path, &value).await.expect("write");
        let loaded: serde_json::Value = read_json_async(&path).await.expect("read");

        assert_eq!(loaded, value);
        let _ = tokio::fs::remove_file(&path).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn write_bytes_private_atomic_sets_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir =
            std::env::temp_dir().join(format!("warp-agentd-fs-async-perms-{}", unique_suffix()));
        let path = dir.join("cred.json");
        write_bytes_private_atomic_async(&path, b"secret")
            .await
            .expect("write");

        let dir_mode = tokio::fs::metadata(&dir)
            .await
            .expect("dir metadata")
            .permissions()
            .mode()
            & 0o777;
        let file_mode = tokio::fs::metadata(&path)
            .await
            .expect("file metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
        assert_eq!(file_mode, 0o600);

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
