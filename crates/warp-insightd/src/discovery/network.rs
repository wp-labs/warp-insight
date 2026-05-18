//! Host-side network inventory discovery probe.

use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use std::fs;
use std::io;
#[cfg(unix)]
use std::{
    ffi::CStr,
    net::{Ipv4Addr, Ipv6Addr},
    ptr,
};

use warp_insight_contracts::discovery::{DiscoveredResource, DiscoveredTarget, DiscoveryOrigin};
use warp_insight_shared::time::now_rfc3339;

use super::host::{default_host_id, default_host_name};
use super::{DiscoveryProbe, DiscoveryProbeError, DiscoverySourceKind, ProbeOutput};

pub struct NetworkDiscoveryProbe;

impl DiscoveryProbe for NetworkDiscoveryProbe {
    fn name(&self) -> &'static str {
        "network"
    }

    fn source(&self) -> DiscoverySourceKind {
        DiscoverySourceKind::LocalRuntime
    }

    fn refresh_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(300)
    }

    fn refresh(&self, _now: std::time::SystemTime) -> Result<ProbeOutput, DiscoveryProbeError> {
        let discovered_at = now_rfc3339();
        let host_id = default_host_id();
        let host_name = default_host_name();
        let source = self.source().as_str().to_string();
        let observed_at = discovered_at.clone();
        let origin_id = format!("{}:{}:{}", source, self.name(), observed_at);
        let inventory = discover_network_inventory().map_err(|err| {
            DiscoveryProbeError::new(
                self.name(),
                self.source(),
                format!("network discovery failed: {err}"),
            )
        })?;

        let mut resources = Vec::new();
        let mut targets = Vec::new();
        for iface in inventory {
            let iface_id = format!("{host_id}:if:{}", iface.name);
            let mut iface_attrs = BTreeMap::from([
                ("host.id".to_string(), host_id.clone()),
                ("host.name".to_string(), host_name.clone()),
                ("net.if.name".to_string(), iface.name.clone()),
            ]);
            if let Some(mac) = &iface.mac {
                iface_attrs.insert("net.if.mac".to_string(), mac.clone());
            }
            if let Some(state) = &iface.state {
                iface_attrs.insert("net.if.state".to_string(), state.clone());
            }

            resources.push(DiscoveredResource {
                resource_id: iface_id.clone(),
                kind: "network_interface".to_string(),
                origin_idx: 0,
                attributes: iface_attrs,
                discovered_at: discovered_at.clone(),
                last_seen_at: discovered_at.clone(),
                health: "healthy".to_string(),
                source: self.name().to_string(),
            });
            targets.push(DiscoveredTarget {
                target_id: format!("{iface_id}:network_interface"),
                kind: "network_interface".to_string(),
                origin_idx: 0,
                resource_ref: iface_id.clone(),
                execution_hints: BTreeMap::from([
                    ("host.id".to_string(), host_id.clone()),
                    ("host.name".to_string(), host_name.clone()),
                    ("net.if.name".to_string(), iface.name.clone()),
                ]),
                state: "active".to_string(),
            });

            for address in iface.addresses {
                let address_id = format!("{}:ip:{}", iface_id, encode_id_segment(&address.ip));
                let cidr = address.cidr();
                let mut attrs = BTreeMap::from([
                    ("host.id".to_string(), host_id.clone()),
                    ("host.name".to_string(), host_name.clone()),
                    ("net.if.name".to_string(), iface.name.clone()),
                    ("net.if.addr".to_string(), address.ip.clone()),
                    ("net.if.prefix".to_string(), address.prefix.to_string()),
                    ("net.if.cidr".to_string(), cidr.clone()),
                    ("network.interface.ref".to_string(), iface_id.clone()),
                ]);
                if let Some(gateway) = &address.gateway_ip {
                    attrs.insert("net.if.gateway".to_string(), gateway.clone());
                }
                if let Some(scope) = &address.scope {
                    attrs.insert("net.if.addr.scope".to_string(), scope.clone());
                }
                if let Some(mac) = &iface.mac {
                    attrs.insert("net.if.mac".to_string(), mac.clone());
                }

                resources.push(DiscoveredResource {
                    resource_id: address_id.clone(),
                    kind: "ip_address".to_string(),
                    origin_idx: 0,
                    attributes: attrs.clone(),
                    discovered_at: discovered_at.clone(),
                    last_seen_at: discovered_at.clone(),
                    health: "healthy".to_string(),
                    source: self.name().to_string(),
                });
                targets.push(DiscoveredTarget {
                    target_id: format!("{address_id}:ip_address"),
                    kind: "ip_address".to_string(),
                    origin_idx: 0,
                    resource_ref: address_id,
                    execution_hints: attrs,
                    state: "active".to_string(),
                });
            }
        }

        Ok(ProbeOutput {
            probe: self.name().to_string(),
            source: self.source(),
            refreshed_at: discovered_at,
            origin: DiscoveryOrigin {
                origin_id,
                probe: self.name().to_string(),
                source,
                observed_at,
            },
            resources,
            targets,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedNetworkInterface {
    name: String,
    mac: Option<String>,
    state: Option<String>,
    addresses: Vec<ObservedIpAddress>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedIpAddress {
    ip: String,
    prefix: u8,
    gateway_ip: Option<String>,
    scope: Option<String>,
}

impl ObservedIpAddress {
    fn cidr(&self) -> String {
        format!("{}/{}", self.ip, self.prefix)
    }
}

#[cfg(target_os = "linux")]
fn discover_network_inventory() -> io::Result<Vec<ObservedNetworkInterface>> {
    let gateways = linux_default_gateways()?;
    let mut interfaces = unix_interfaces_from_getifaddrs(&gateways)?;
    for iface in &mut interfaces {
        iface.mac = linux_read_interface_file(&iface.name, "address")
            .filter(|mac| mac != "00:00:00:00:00:00");
        iface.state = linux_read_interface_file(&iface.name, "operstate");
    }
    Ok(interfaces)
}

#[cfg(target_os = "linux")]
fn linux_default_gateways() -> io::Result<BTreeMap<String, String>> {
    fs::read_to_string("/proc/net/route")
        .map(|content| parse_linux_proc_default_gateways(&content))
        .or_else(|err| {
            if err.kind() == io::ErrorKind::NotFound {
                Ok(BTreeMap::new())
            } else {
                Err(err)
            }
        })
}

#[cfg(target_os = "linux")]
fn linux_read_interface_file(iface: &str, file: &str) -> Option<String> {
    fs::read_to_string(format!("/sys/class/net/{iface}/{file}"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(target_os = "linux")]
fn parse_linux_proc_default_gateways(content: &str) -> BTreeMap<String, String> {
    content
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() < 3 || fields[1] != "00000000" {
                return None;
            }
            let iface = fields[0].to_string();
            let gateway = u32::from_str_radix(fields[2], 16).ok()?;
            let octets = gateway.to_le_bytes();
            Some((iface, Ipv4Addr::from(octets).to_string()))
        })
        .collect()
}

#[cfg(all(unix, not(target_os = "linux")))]
fn discover_network_inventory() -> io::Result<Vec<ObservedNetworkInterface>> {
    unix_interfaces_from_getifaddrs(&BTreeMap::new())
}

#[cfg(unix)]
fn unix_interfaces_from_getifaddrs(
    gateways: &BTreeMap<String, String>,
) -> io::Result<Vec<ObservedNetworkInterface>> {
    let mut addrs: *mut libc::ifaddrs = ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut addrs) } != 0 {
        return Err(io::Error::last_os_error());
    }

    let mut interfaces: BTreeMap<String, ObservedNetworkInterface> = BTreeMap::new();
    let result = {
        let mut cursor = addrs;
        while !cursor.is_null() {
            let item = unsafe { &*cursor };
            cursor = item.ifa_next;
            if item.ifa_addr.is_null() {
                continue;
            }
            let flags = item.ifa_flags;
            if flags & (libc::IFF_LOOPBACK as u32) != 0 {
                continue;
            }
            let name = unsafe { CStr::from_ptr(item.ifa_name) }
                .to_string_lossy()
                .to_string();
            let iface =
                interfaces
                    .entry(name.clone())
                    .or_insert_with(|| ObservedNetworkInterface {
                        name: name.clone(),
                        mac: None,
                        state: Some(if flags & (libc::IFF_UP as u32) != 0 {
                            "up".to_string()
                        } else {
                            "down".to_string()
                        }),
                        addresses: Vec::new(),
                    });

            let family = unsafe { (*item.ifa_addr).sa_family as i32 };
            match family {
                libc::AF_INET => {
                    let sockaddr = unsafe { &*(item.ifa_addr as *const libc::sockaddr_in) };
                    let ip = Ipv4Addr::from(u32::from_be(sockaddr.sin_addr.s_addr)).to_string();
                    if ip.starts_with("127.") {
                        continue;
                    }
                    let prefix = if item.ifa_netmask.is_null() {
                        32
                    } else {
                        let netmask = unsafe { &*(item.ifa_netmask as *const libc::sockaddr_in) };
                        u32::from_be(netmask.sin_addr.s_addr).count_ones() as u8
                    };
                    iface.addresses.push(ObservedIpAddress {
                        ip,
                        prefix,
                        gateway_ip: gateways.get(&name).cloned(),
                        scope: None,
                    });
                }
                libc::AF_INET6 => {
                    let sockaddr = unsafe { &*(item.ifa_addr as *const libc::sockaddr_in6) };
                    let ip = Ipv6Addr::from(sockaddr.sin6_addr.s6_addr).to_string();
                    if ip == "::1" {
                        continue;
                    }
                    let prefix = if item.ifa_netmask.is_null() {
                        128
                    } else {
                        let netmask = unsafe { &*(item.ifa_netmask as *const libc::sockaddr_in6) };
                        netmask
                            .sin6_addr
                            .s6_addr
                            .iter()
                            .map(|octet| octet.count_ones())
                            .sum::<u32>() as u8
                    };
                    iface.addresses.push(ObservedIpAddress {
                        scope: ipv6_scope(&ip).map(str::to_string),
                        ip,
                        prefix,
                        gateway_ip: None,
                    });
                }
                _ => {}
            }
        }

        Ok(interfaces
            .into_values()
            .filter(|iface| !iface.addresses.is_empty())
            .collect())
    };
    unsafe { libc::freeifaddrs(addrs) };
    result
}

fn encode_id_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len() * 2);
    for byte in value.as_bytes() {
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}

fn ipv6_scope(ip: &str) -> Option<&'static str> {
    let addr = ip.parse::<Ipv6Addr>().ok()?;
    if addr.is_unicast_link_local() {
        Some("link_local")
    } else if addr.is_unique_local() {
        Some("unique_local")
    } else {
        None
    }
}

#[cfg(not(unix))]
fn discover_network_inventory() -> io::Result<Vec<ObservedNetworkInterface>> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::ObservedIpAddress;
    #[cfg(target_os = "linux")]
    use super::parse_linux_proc_default_gateways;
    use super::{encode_id_segment, ipv6_scope};

    #[test]
    fn address_formats_cidr() {
        let address = ObservedIpAddress {
            ip: "192.168.10.41".to_string(),
            prefix: 24,
            gateway_ip: None,
            scope: None,
        };

        assert_eq!(address.cidr(), "192.168.10.41/24");
    }

    #[test]
    fn address_id_segments_are_encoded() {
        assert_eq!(encode_id_segment("fe80::1"), "666538303a3a31");
    }

    #[test]
    fn classifies_ipv6_address_scope() {
        assert_eq!(ipv6_scope("fe80::1"), Some("link_local"));
        assert_eq!(ipv6_scope("fd00::1"), Some("unique_local"));
        assert_eq!(ipv6_scope("2001:db8::1"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_linux_proc_default_gateways_by_interface() {
        let gateways = parse_linux_proc_default_gateways(
            "Iface\tDestination\tGateway\tFlags\tRefCnt\tUse\tMetric\tMask\n\
             eth0\t00000000\t010AA8C0\t0003\t0\t0\t100\t00000000\n\
             wlan0\t00000000\t0101A8C0\t0003\t0\t0\t200\t00000000\n",
        );

        assert_eq!(
            gateways.get("eth0").map(String::as_str),
            Some("192.168.10.1")
        );
        assert_eq!(
            gateways.get("wlan0").map(String::as_str),
            Some("192.168.1.1")
        );
    }
}
