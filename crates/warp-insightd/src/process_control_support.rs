use std::io;
#[cfg(unix)]
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProcessIdentityState {
    MissingProcess,
    Matches,
    Mismatch,
    Unavailable,
}

pub(super) fn process_identity_state(
    pid: u32,
    expected: Option<&str>,
) -> io::Result<ProcessIdentityState> {
    if !process_exists(pid)? {
        return Ok(ProcessIdentityState::MissingProcess);
    }

    let actual = match expected {
        Some(_) => process_identity(pid)?,
        None => None,
    };
    Ok(classify_process_identity(expected, actual.as_deref()))
}

pub(super) fn classify_process_identity(
    expected: Option<&str>,
    actual: Option<&str>,
) -> ProcessIdentityState {
    match expected {
        None => ProcessIdentityState::Matches,
        Some(expected) => match actual {
            Some(actual) if actual == expected => ProcessIdentityState::Matches,
            Some(_) => ProcessIdentityState::Mismatch,
            None => ProcessIdentityState::Unavailable,
        },
    }
}

pub(super) fn derive_running_state_status(
    identity_state: ProcessIdentityState,
    deadline_elapsed: bool,
) -> crate::process_control::RunningStateStatus {
    match identity_state {
        ProcessIdentityState::MissingProcess | ProcessIdentityState::Mismatch => {
            crate::process_control::RunningStateStatus::Inactive
        }
        ProcessIdentityState::Unavailable => crate::process_control::RunningStateStatus::Active,
        ProcessIdentityState::Matches if deadline_elapsed => {
            crate::process_control::RunningStateStatus::Expired
        }
        ProcessIdentityState::Matches => crate::process_control::RunningStateStatus::Active,
    }
}

pub(crate) fn process_identity(pid: u32) -> io::Result<Option<String>> {
    if pid == 0 {
        return Ok(None);
    }
    process_identity_token(pid)
}

#[cfg(unix)]
pub(crate) fn send_terminate(pid: u32) -> io::Result<()> {
    send_signal(pid, libc::SIGTERM)
}

#[cfg(not(unix))]
pub(crate) fn send_terminate(_pid: u32) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
pub(crate) fn force_kill(pid: u32) -> io::Result<bool> {
    if pid == 0 {
        return Ok(false);
    }

    match send_signal(pid, libc::SIGKILL) {
        Ok(()) => wait_for_exit(pid, Duration::from_millis(250)),
        Err(err) if err.raw_os_error() == Some(libc::EPERM) => Ok(true),
        Err(err) if err.raw_os_error() == Some(libc::ESRCH) => Ok(false),
        Err(err) => Err(err),
    }
}

#[cfg(not(unix))]
pub(crate) fn force_kill(_pid: u32) -> io::Result<bool> {
    Ok(false)
}

#[cfg(unix)]
pub(super) fn process_exists(pid: u32) -> io::Result<bool> {
    if pid == 0 {
        return Ok(false);
    }

    // SAFETY: `kill(pid, 0)` only asks the kernel to check process existence and
    // permission. It does not dereference pointers or access Rust-managed memory.
    let rc = unsafe { libc::kill(pid as i32, 0) };
    if rc == 0 {
        return Ok(true);
    }

    let err = io::Error::last_os_error();
    match err.raw_os_error() {
        Some(code) if code == libc::ESRCH => Ok(false),
        Some(code) if code == libc::EPERM => Ok(true),
        _ => Err(err),
    }
}

#[cfg(not(unix))]
pub(super) fn process_exists(_pid: u32) -> io::Result<bool> {
    Ok(false)
}

#[cfg(unix)]
fn send_signal(pid: u32, signal: i32) -> io::Result<()> {
    if pid == 0 {
        return Ok(());
    }

    // SAFETY: `pid` is an OS process identifier from child state or persisted
    // runtime state. Sending a signal has no Rust memory-safety implications.
    let rc = unsafe { libc::kill(pid as i32, signal) };
    if rc == 0 {
        return Ok(());
    }

    Err(io::Error::last_os_error())
}

#[cfg(unix)]
fn wait_for_exit(pid: u32, timeout: Duration) -> io::Result<bool> {
    let deadline = Instant::now() + timeout;
    loop {
        if !process_exists(pid)? {
            return Ok(false);
        }
        if process_is_zombie(pid)? {
            return Ok(false);
        }
        if Instant::now() >= deadline {
            return Ok(true);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "linux")]
fn process_identity_token(pid: u32) -> io::Result<Option<String>> {
    let stat_path = format!("/proc/{pid}/stat");
    let stat = match std::fs::read_to_string(stat_path) {
        Ok(stat) => stat,
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            ) =>
        {
            return Ok(None);
        }
        Err(err) => return Err(err),
    };
    let Some((_, tail)) = stat.rsplit_once(") ") else {
        return Ok(None);
    };
    let fields: Vec<&str> = tail.split_whitespace().collect();
    if fields.len() <= 19 {
        return Ok(None);
    }
    Ok(Some(format!("linux_proc_start:{}", fields[19])))
}

#[cfg(all(unix, not(target_os = "linux")))]
fn process_identity_token(pid: u32) -> io::Result<Option<String>> {
    let output = match Command::new("ps")
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .output()
    {
        Ok(output) => output,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return Ok(None),
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if !output.status.success() {
        return Ok(None);
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!("ps_lstart:{token}")))
}

#[cfg(not(unix))]
fn process_identity_token(_pid: u32) -> io::Result<Option<String>> {
    Ok(None)
}

#[cfg(target_os = "linux")]
fn process_is_zombie(pid: u32) -> io::Result<bool> {
    let stat_path = format!("/proc/{pid}/stat");
    let stat = match std::fs::read_to_string(stat_path) {
        Ok(stat) => stat,
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            ) =>
        {
            return Ok(false);
        }
        Err(err) => return Err(err),
    };
    Ok(parse_linux_proc_state(&stat) == Some('Z'))
}

#[cfg(target_os = "linux")]
pub(super) fn parse_linux_proc_state(stat: &str) -> Option<char> {
    let (_, tail) = stat.rsplit_once(") ")?;
    tail.chars().next()
}

#[cfg(all(unix, not(target_os = "linux")))]
fn process_is_zombie(pid: u32) -> io::Result<bool> {
    process_is_zombie_via_ps(pid, "ps")
}

#[cfg(all(unix, not(target_os = "linux")))]
pub(super) fn process_is_zombie_via_ps(pid: u32, program: &str) -> io::Result<bool> {
    let output = match Command::new(program)
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
    {
        Ok(output) => output,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return Ok(false),
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };
    if !output.status.success() {
        return Ok(false);
    }
    let stat = String::from_utf8_lossy(&output.stdout);
    Ok(stat.trim_start().starts_with('Z'))
}

#[cfg(not(unix))]
fn process_is_zombie(_pid: u32) -> io::Result<bool> {
    Ok(false)
}
