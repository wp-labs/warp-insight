use std::{
    collections::HashMap,
    error, fmt, fs,
    fs::OpenOptions,
    io,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock, Weak},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AdminStore {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "admin store io error: {err}"),
            Self::Json(err) => write!(f, "admin store json error: {err}"),
        }
    }
}

impl error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AdminStoreSnapshot {
    pub enrollment_tokens: HashMap<String, StoredEnrollmentToken>,
    pub agents: HashMap<String, StoredAgentRegistration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEnrollmentToken {
    pub token_hash: String,
    pub tenant_id: String,
    pub environment_id: String,
    pub max_uses: u32,
    pub used_count: u32,
    pub issued_at: String,
    pub expires_at: String,
    #[serde(default)]
    pub reserved_at: Option<String>,
    pub status: StoredEnrollmentTokenStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredEnrollmentTokenStatus {
    Active,
    Reserved,
    Used,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StoredAgentRegistration {
    pub agent_id: String,
    pub instance_id: String,
    pub tenant_id: String,
    pub environment_id: String,
    pub node_id: String,
    pub hostname: String,
    pub machine_id: String,
    pub version: String,
    pub credential_id: String,
    pub credential_token_hash: String,
    pub credential_issued_at: String,
    pub credential_expires_at: String,
    pub credential_status: StoredCredentialStatus,
    pub registered_at: String,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredCredentialStatus {
    Active,
    Expired,
    Revoked,
}

impl Default for StoredCredentialStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl AdminStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let lock = shared_process_lock(&path);
        Self { path, lock }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<AdminStoreSnapshot, StoreError> {
        let _guard = self.lock()?;
        self.load_snapshot()
    }

    pub fn save(&self, snapshot: &AdminStoreSnapshot) -> Result<(), StoreError> {
        let _guard = self.lock()?;
        self.save_snapshot(snapshot)
    }

    pub fn update<T>(
        &self,
        mutate: impl FnOnce(&mut AdminStoreSnapshot) -> T,
    ) -> Result<T, StoreError> {
        let _guard = self.lock()?;
        let mut snapshot = self.load_snapshot()?;
        let output = mutate(&mut snapshot);
        self.save_snapshot(&snapshot)?;
        Ok(output)
    }

    pub fn update_result<T, E>(
        &self,
        mutate: impl FnOnce(&mut AdminStoreSnapshot) -> Result<T, E>,
    ) -> Result<Result<T, E>, StoreError> {
        let _guard = self.lock()?;
        let mut snapshot = self.load_snapshot()?;
        match mutate(&mut snapshot) {
            Ok(output) => {
                self.save_snapshot(&snapshot)?;
                Ok(Ok(output))
            }
            Err(err) => Ok(Err(err)),
        }
    }

    fn lock(&self) -> Result<StoreLockGuard<'_>, StoreError> {
        let process_guard = self
            .lock
            .lock()
            .map_err(|_| StoreError::Io(io::Error::other("admin store lock poisoned")))?;
        let file_guard = FileLockGuard::lock(&lock_file_path(&self.path))?;
        Ok(StoreLockGuard {
            _process_guard: process_guard,
            _file_guard: file_guard,
        })
    }

    fn load_snapshot(&self) -> Result<AdminStoreSnapshot, StoreError> {
        if !self.path.exists() {
            return Ok(AdminStoreSnapshot::default());
        }
        let content = fs::read_to_string(&self.path)?;
        if content.trim().is_empty() {
            return Ok(AdminStoreSnapshot::default());
        }
        Ok(serde_json::from_str(&content)?)
    }

    fn save_snapshot(&self, snapshot: &AdminStoreSnapshot) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(snapshot)?;
        let temp_path = temp_store_path(&self.path);
        let write_result = (|| -> Result<(), StoreError> {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temp_path)?;
            file.write_all(content.as_bytes())?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            fs::rename(&temp_path, &self.path)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))?;
            }
            if let Some(parent) = self.path.parent() {
                sync_directory(parent)?;
            }
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result?;
        Ok(())
    }
}

struct StoreLockGuard<'a> {
    _process_guard: MutexGuard<'a, ()>,
    _file_guard: FileLockGuard,
}

static PROCESS_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();

fn shared_process_lock(path: &Path) -> Arc<Mutex<()>> {
    let key = normalized_store_path(path);
    let locks = PROCESS_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = locks.lock().expect("admin store process lock map poisoned");
    if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(key, Arc::downgrade(&lock));
    lock
}

fn normalized_store_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn lock_file_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("admin-store.json");
    path.with_file_name(format!(".{file_name}.lock"))
}

fn temp_store_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("admin-store.json");
    let nanos = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| chrono::Utc::now().timestamp_micros() * 1_000);
    path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), nanos))
}

#[cfg(unix)]
struct FileLockGuard {
    file: fs::File,
}

#[cfg(unix)]
impl FileLockGuard {
    fn lock(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        lock_file_exclusive(&file)?;
        Ok(Self { file })
    }
}

#[cfg(unix)]
impl Drop for FileLockGuard {
    fn drop(&mut self) {
        let _ = unlock_file(&self.file);
    }
}

#[cfg(not(unix))]
struct FileLockGuard;

#[cfg(not(unix))]
impl FileLockGuard {
    fn lock(_path: &Path) -> Result<Self, StoreError> {
        Ok(Self)
    }
}

#[cfg(unix)]
fn lock_file_exclusive(file: &fs::File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    const LOCK_EX: std::os::raw::c_int = 2;
    if unsafe { flock(file.as_raw_fd(), LOCK_EX) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(unix)]
fn unlock_file(file: &fs::File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    const LOCK_UN: std::os::raw::c_int = 8;
    if unsafe { flock(file.as_raw_fd(), LOCK_UN) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(unix)]
extern "C" {
    fn flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int;
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        OpenOptions::new().read(true).open(path)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, Barrier},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn admin_store_updates_are_serialized_across_same_path_instances() {
        let path = test_store_path();
        let barrier = Arc::new(Barrier::new(16));
        let mut handles = Vec::new();

        for index in 0..16 {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                let store = AdminStore::new(path);
                barrier.wait();
                store
                    .update(|snapshot| {
                        snapshot.enrollment_tokens.insert(
                            format!("token-{index}"),
                            StoredEnrollmentToken {
                                token_hash: format!("token-{index}"),
                                tenant_id: "tenant".to_string(),
                                environment_id: "env".to_string(),
                                max_uses: 1,
                                used_count: 0,
                                issued_at: "2026-07-30T00:00:00Z".to_string(),
                                expires_at: "2026-07-31T00:00:00Z".to_string(),
                                reserved_at: None,
                                status: StoredEnrollmentTokenStatus::Active,
                            },
                        );
                    })
                    .expect("store update");
            }));
        }

        for handle in handles {
            handle.join().expect("thread finished");
        }

        let snapshot = AdminStore::new(&path).load().expect("store load");
        assert_eq!(snapshot.enrollment_tokens.len(), 16);
    }

    #[test]
    fn admin_store_update_result_rolls_back_failed_mutation() {
        let path = test_store_path();
        let store = AdminStore::new(&path);
        let result = store
            .update_result(|snapshot| {
                snapshot.enrollment_tokens.insert(
                    "token-a".to_string(),
                    StoredEnrollmentToken {
                        token_hash: "token-a".to_string(),
                        tenant_id: "tenant".to_string(),
                        environment_id: "env".to_string(),
                        max_uses: 1,
                        used_count: 0,
                        issued_at: "2026-07-30T00:00:00Z".to_string(),
                        expires_at: "2026-07-31T00:00:00Z".to_string(),
                        reserved_at: None,
                        status: StoredEnrollmentTokenStatus::Active,
                    },
                );
                Err::<(), _>("reject")
            })
            .expect("store update result");

        assert_eq!(result, Err("reject"));
        let snapshot = store.load().expect("store load");
        assert!(snapshot.enrollment_tokens.is_empty());
    }

    fn test_store_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("warp-insight-store-test-{nanos}.json"))
    }
}
