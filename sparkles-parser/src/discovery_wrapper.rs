use std::collections::{BTreeMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::thread;
use std::time::Duration;
use log::info;
use multicast_discovery_socket::config::MulticastDiscoveryConfig;
use multicast_discovery_socket::MulticastDiscoverySocket;


const DEFAULT_MULTICAST_GROUP: Ipv4Addr = Ipv4Addr::new(239, 38, 38, 38);
pub fn default_config() -> MulticastDiscoveryConfig {
    MulticastDiscoveryConfig::new(DEFAULT_MULTICAST_GROUP, "sparkles".into())
        .with_multicast_port(38338)
        .with_backup_ports(45_337..45_339)
        .with_disabled_announce()
}

pub struct DiscoveryWrapper {
    socket: MulticastDiscoverySocket<()>,
}

impl DiscoveryWrapper {
    pub fn new() -> Self {
        let cfg = default_config();
        let mut socket = MulticastDiscoverySocket::new_discover_only(&cfg)
            .expect("Failed to create discovery socket");
        
        socket.set_discover_replies_en(false);
        Self { socket }
    }
    
    pub fn discover(&mut self) -> std::io::Result<BTreeMap<u32, Vec<SocketAddr>>> {
        
        self.socket.discover();
        thread::sleep(Duration::from_millis(50));
        self.socket.discover();

        let mut clients: BTreeMap<u32, HashSet<SocketAddr>> = BTreeMap::new();
        for _ in 0..10 {
            self.socket.poll(|res| {
                match res {
                    multicast_discovery_socket::PollResult::DiscoveredClient { addr, discover_id, .. } => {
                        clients.entry(discover_id).or_default().insert(addr.into());
                    }
                    multicast_discovery_socket::PollResult::DisconnectedClient { addr, discover_id } => {
                        info!("Client disconnected: {addr} - {discover_id:x}");
                    }
                }
            });
            thread::sleep(Duration::from_millis(50));
        }

        // Heuristic sorting of addresses by priority
        Ok(clients.iter_mut().map(|(id, addrs)| {
            let mut addrs: Vec<_> = addrs.iter().cloned().collect();
            addrs.sort_by_key(|a1| {
                if a1.is_ipv4() {
                    match a1.ip() {
                        IpAddr::V4(a) => {
                            match a.octets() {
                                [127, _, _, _] => 0,
                                [192, 168, _, _] => 10,
                                [172, b, _, _] if (16..=31).contains(&b) => 20,
                                [10, _, _, _] => 30,
                                _ => 90
                            }
                        }
                        _ => 100
                    }
                } else if a1.ip().is_loopback() {
                    80
                } else {
                    100
                }
            });
            (*id, addrs)
        }).collect())
    }
}