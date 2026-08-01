//! Linux `/proc` implementation for service endpoint discovery.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::process_control::process_identity;

use super::ObservedServiceEndpoint;

pub(super) fn discover_service_endpoints() -> io::Result<Vec<ObservedServiceEndpoint>> {
    let mut endpoints = Vec::new();
    for (path, protocol, family, table_kind) in [
        (
            "/proc/net/tcp",
            "tcp",
            AddressFamily::Inet4,
            SocketTableKind::TcpListen,
        ),
        (
            "/proc/net/tcp6",
            "tcp",
            AddressFamily::Inet6,
            SocketTableKind::TcpListen,
        ),
        (
            "/proc/net/udp",
            "udp",
            AddressFamily::Inet4,
            SocketTableKind::UdpBound,
        ),
        (
            "/proc/net/udp6",
            "udp",
            AddressFamily::Inet6,
            SocketTableKind::UdpBound,
        ),
    ] {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        endpoints.extend(parse_linux_socket_table(
            &content, protocol, family, table_kind,
        ));
    }

    let inodes: BTreeSet<_> = endpoints
        .iter()
        .map(|endpoint| endpoint.socket_inode.clone())
        .collect();
    let owners = map_socket_inodes_to_processes(&inodes)?;
    for endpoint in &mut endpoints {
        if let Some(socket_owners) = owners.get(&endpoint.socket_inode) {
            let Some(owner) = socket_owners.first() else {
                continue;
            };
            endpoint.pid = Some(owner.pid);
            endpoint.process_identity = owner.process_identity.clone();
            endpoint.process_name = owner.process_name.clone();
            endpoint.cgroup_path = owner.cgroup_path.clone();
            endpoint.container_id = owner.container_id.clone();
            endpoint.owner_count = Some(socket_owners.len());
            endpoint.binding_confidence = Some(if socket_owners.len() == 1 {
                "single_owner".to_string()
            } else {
                "shared_socket".to_string()
            });
        }
    }
    Ok(endpoints)
}

#[derive(Debug, Clone, Copy)]
enum AddressFamily {
    Inet4,
    Inet6,
}

#[derive(Debug, Clone, Copy)]
enum SocketTableKind {
    TcpListen,
    UdpBound,
}

#[derive(Debug, Clone, PartialEq, Eq, ::moju_derive::MoJu)]
#[moju(kind = "struct", domain = "Observed", module = "Observed.Entity")]
struct ProcessSocketOwner {
    pid: u32,
    process_identity: Option<String>,
    process_name: Option<String>,
    cgroup_path: Option<String>,
    container_id: Option<String>,
}

fn parse_linux_socket_table(
    content: &str,
    protocol: &str,
    family: AddressFamily,
    table_kind: SocketTableKind,
) -> Vec<ObservedServiceEndpoint> {
    content
        .lines()
        .skip(1)
        .filter_map(|line| parse_linux_socket_line(line, protocol, family, table_kind))
        .collect()
}

fn parse_linux_socket_line(
    line: &str,
    protocol: &str,
    family: AddressFamily,
    table_kind: SocketTableKind,
) -> Option<ObservedServiceEndpoint> {
    let fields: Vec<_> = line.split_whitespace().collect();
    if fields.len() <= 9 {
        return None;
    }
    if matches!(table_kind, SocketTableKind::TcpListen) && fields[3] != "0A" {
        return None;
    }
    let (local_addr, local_port) = fields[1].split_once(':')?;
    let (remote_addr, remote_port) = fields[2].split_once(':')?;
    if matches!(table_kind, SocketTableKind::UdpBound)
        && (!is_unspecified_proc_addr(remote_addr) || remote_port != "0000")
    {
        return None;
    }
    let local_ip = match family {
        AddressFamily::Inet4 => parse_proc_ipv4(local_addr)?,
        AddressFamily::Inet6 => parse_proc_ipv6(local_addr)?,
    };
    let local_port = u16::from_str_radix(local_port, 16).ok()?;
    if local_port == 0 {
        return None;
    }
    let socket_inode = fields[9].to_string();
    if socket_inode == "0" {
        return None;
    }

    Some(ObservedServiceEndpoint {
        protocol: protocol.to_string(),
        local_ip,
        local_port,
        socket_inode,
        pid: None,
        process_identity: None,
        process_name: None,
        cgroup_path: None,
        container_id: None,
        owner_count: None,
        binding_confidence: None,
    })
}

fn map_socket_inodes_to_processes(
    inodes: &BTreeSet<String>,
) -> io::Result<BTreeMap<String, Vec<ProcessSocketOwner>>> {
    let mut inode_pids: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    if inodes.is_empty() {
        return Ok(BTreeMap::new());
    }

    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(file_name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Ok(pid) = file_name.parse::<u32>() else {
            continue;
        };
        let fd_dir = entry.path().join("fd");
        let fds = match fs::read_dir(fd_dir) {
            Ok(fds) => fds,
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
                ) =>
            {
                continue;
            }
            Err(err) => return Err(err),
        };
        for fd in fds {
            let fd = match fd {
                Ok(fd) => fd,
                Err(err)
                    if matches!(
                        err.kind(),
                        io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
                    ) =>
                {
                    continue;
                }
                Err(err) => return Err(err),
            };
            let target = match fs::read_link(fd.path()) {
                Ok(target) => target,
                Err(err)
                    if matches!(
                        err.kind(),
                        io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
                    ) =>
                {
                    continue;
                }
                Err(err) => return Err(err),
            };
            let Some(inode) = socket_inode_from_link(&target.to_string_lossy()) else {
                continue;
            };
            if inodes.contains(&inode) {
                inode_pids.entry(inode).or_default().insert(pid);
            }
        }
    }

    let mut owner_cache = BTreeMap::new();
    let mut owners = BTreeMap::new();
    for (inode, pids) in inode_pids {
        let socket_owners = pids
            .into_iter()
            .map(|pid| {
                owner_cache
                    .entry(pid)
                    .or_insert_with(|| process_owner(pid))
                    .clone()
            })
            .collect();
        owners.insert(inode, socket_owners);
    }
    Ok(owners)
}

fn process_owner(pid: u32) -> ProcessSocketOwner {
    let cgroup_path = process_cgroup_path(pid);
    ProcessSocketOwner {
        pid,
        process_identity: process_identity(pid).ok().flatten(),
        process_name: process_name(pid),
        container_id: cgroup_path
            .as_deref()
            .and_then(container_id_from_cgroup_path),
        cgroup_path,
    }
}

fn parse_proc_ipv4(value: &str) -> Option<String> {
    let raw = u32::from_str_radix(value, 16).ok()?;
    Some(Ipv4Addr::from(raw.to_le_bytes()).to_string())
}

fn is_unspecified_proc_addr(value: &str) -> bool {
    value.chars().all(|ch| ch == '0')
}

fn parse_proc_ipv6(value: &str) -> Option<String> {
    if value.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for (chunk_idx, chunk) in value.as_bytes().chunks(8).enumerate() {
        let text = std::str::from_utf8(chunk).ok()?;
        let raw = u32::from_str_radix(text, 16).ok()?;
        bytes[chunk_idx * 4..chunk_idx * 4 + 4].copy_from_slice(&raw.to_le_bytes());
    }
    Some(Ipv6Addr::from(bytes).to_string())
}

fn socket_inode_from_link(link: &str) -> Option<String> {
    link.strip_prefix("socket:[")?
        .strip_suffix(']')
        .filter(|inode| !inode.is_empty())
        .map(str::to_string)
}

fn process_name(pid: u32) -> Option<String> {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn process_cgroup_path(pid: u32) -> Option<String> {
    let content = fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    let paths: Vec<_> = content
        .lines()
        .filter_map(|line| line.rsplit_once(':').map(|(_, path)| path.trim()))
        .filter(|path| !path.is_empty() && *path != "/")
        .collect();
    paths
        .iter()
        .find(|path| container_id_from_cgroup_path(path).is_some())
        .or_else(|| paths.first())
        .map(|path| (*path).to_string())
}

fn container_id_from_cgroup_path(path: &str) -> Option<String> {
    path.split(['/', ':'])
        .rev()
        .find_map(container_id_from_cgroup_segment)
}

fn container_id_from_cgroup_segment(segment: &str) -> Option<String> {
    let trimmed = segment.trim().trim_end_matches(".scope");
    for prefix in ["cri-containerd-", "docker-", "crio-"] {
        if let Some(value) = trimmed.strip_prefix(prefix) {
            return valid_container_id_prefix(value);
        }
    }
    valid_container_id_prefix(trimmed)
}

fn valid_container_id_prefix(value: &str) -> Option<String> {
    let hex: String = value
        .chars()
        .take_while(|ch| ch.is_ascii_hexdigit())
        .collect();
    if hex.len() >= 12 { Some(hex) } else { None }
}

#[cfg(test)]
mod tests {
    use super::{
        AddressFamily, SocketTableKind, container_id_from_cgroup_path, is_unspecified_proc_addr,
        parse_linux_socket_line, parse_proc_ipv4, parse_proc_ipv6, socket_inode_from_link,
    };

    #[test]
    fn parses_linux_tcp_listen_line() {
        let line = "0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000 1000 0 12345 1 0000000000000000 100 0 0 10 0";
        let endpoint = parse_linux_socket_line(
            line,
            "tcp",
            AddressFamily::Inet4,
            SocketTableKind::TcpListen,
        )
        .unwrap();

        assert_eq!(endpoint.local_ip, "127.0.0.1");
        assert_eq!(endpoint.local_port, 8080);
        assert_eq!(endpoint.socket_inode, "12345");
    }

    #[test]
    fn ignores_non_listening_tcp_line() {
        let line =
            "0: 0100007F:1F90 0200007F:0050 01 00000000:00000000 00:00000000 00000000 1000 0 12345";

        assert!(
            parse_linux_socket_line(
                line,
                "tcp",
                AddressFamily::Inet4,
                SocketTableKind::TcpListen
            )
            .is_none()
        );
    }

    #[test]
    fn parses_linux_udp_bound_line() {
        let line = "0: 00000000:1F90 00000000:0000 07 00000000:00000000 00:00000000 00000000 1000 0 12346 1 0000000000000000 100 0 0 10 0";
        let endpoint =
            parse_linux_socket_line(line, "udp", AddressFamily::Inet4, SocketTableKind::UdpBound)
                .unwrap();

        assert_eq!(endpoint.protocol, "udp");
        assert_eq!(endpoint.local_ip, "0.0.0.0");
        assert_eq!(endpoint.local_port, 8080);
        assert_eq!(endpoint.socket_inode, "12346");
    }

    #[test]
    fn ignores_connected_udp_line() {
        let line = "0: 00000000:1F90 0100007F:0035 01 00000000:00000000 00:00000000 00000000 1000 0 12346 1 0000000000000000 100 0 0 10 0";

        assert!(
            parse_linux_socket_line(line, "udp", AddressFamily::Inet4, SocketTableKind::UdpBound)
                .is_none()
        );
    }

    #[test]
    fn parses_proc_addresses_and_socket_links() {
        assert_eq!(parse_proc_ipv4("0100007F"), Some("127.0.0.1".to_string()));
        assert_eq!(
            parse_proc_ipv6("00000000000000000000000000000001"),
            Some("::1".to_string())
        );
        assert_eq!(
            socket_inode_from_link("socket:[12345]"),
            Some("12345".to_string())
        );
        assert!(is_unspecified_proc_addr("00000000"));
        assert!(is_unspecified_proc_addr("00000000000000000000000000000000"));
        assert!(!is_unspecified_proc_addr(
            "00000000000000000000000000000001"
        ));
    }

    #[test]
    fn extracts_container_ids_from_cgroup_paths() {
        assert_eq!(
            container_id_from_cgroup_path("/kubepods.slice/cri-containerd-0123456789abcdef.scope"),
            Some("0123456789abcdef".to_string())
        );
        assert_eq!(
            container_id_from_cgroup_path("/docker/0123456789ab"),
            Some("0123456789ab".to_string())
        );
        assert_eq!(container_id_from_cgroup_path("/user.slice"), None);
    }
}
