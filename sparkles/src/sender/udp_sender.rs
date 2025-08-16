use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::{Duration, Instant};
use log::{debug, error, info, trace, warn};
use multicast_discovery_socket::config::MulticastDiscoveryConfig;
use multicast_discovery_socket::MulticastDiscoverySocket;
use sparkles_core::protocol::packets::{PacketType, RequestPacketType};
use sparkles_core::protocol::sender::{ConfiguredSender, PacketFlags, Sender};
use crate::on_client_connect;

const DEFAULT_MULTICAST_GROUP: Ipv4Addr = Ipv4Addr::new(239, 38, 38, 38);

pub fn default_config() -> MulticastDiscoveryConfig {
    MulticastDiscoveryConfig::new(DEFAULT_MULTICAST_GROUP, "sparkles".into())
        .with_multicast_port(38338)
        .with_backup_ports(45_337..45_339)
        .with_disabled_announce()
}

pub(crate) struct UdpSender {
    socket: UdpSocket,
    discovery_socket: Option<MulticastDiscoverySocket<()>>,
    dst_addr: Option<SocketAddr>,
    last_recv: Option<Instant>,
    seq_num: u16,
    
    timestamp_freq_request: Arc<AtomicBool>,
}
impl UdpSender {
    fn new_seq_num(&mut self) -> u16 {
        let seq_num = self.seq_num;
        self.seq_num = self.seq_num.wrapping_add(1);
        if self.seq_num == 0 {
            self.seq_num = 1;
        }
        seq_num
    }
    
    fn try_recv(&mut self) {
        let mut buf = [0u8; 32];
        match self.socket.recv_from(&mut buf) {
            Ok((32, addr)) if buf == RequestPacketType::Subscribe.pattern() => {
                info!("[sparkles] UDP client connected: {addr}");
                if let Some(prev_addr) = self.dst_addr {
                    warn!("[sparkles] Forgetting client: {prev_addr}. Now streaming to {addr}");
                }
                self.dst_addr = Some(addr);
                self.last_recv = Some(Instant::now());
                self.timestamp_freq_request.store(true, std::sync::atomic::Ordering::Relaxed);
                on_client_connect();
                
                if let Err(e) = self.socket.send_to(&PacketType::ConnectionAccepted.pattern(), addr) {
                    warn!("[sparkles] Error sending ConnectionAccepted packet to client: {e:?}");
                }
            }
            Ok(_) => {
                warn!("[sparkles] Incorrect packet received from client! Ignoring...");
            }
            Err(e) => {
                // Got nothing
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    return;
                }
                
                warn!("[sparkles] Error receiving packet from client: {e:?}");
            }
        }
    }
}

const SHORT_PACKET_SIZE: usize = 1300;
#[derive(Debug, Default, Clone)]
pub struct UdpSenderConfig {
    pub desired_port: Option<u16>,
    pub multicast_discovery_config: Option<MulticastDiscoveryConfig>,
}

impl Sender for UdpSender {
    fn send_packet(&mut self, packet_type: PacketType, data: &[&[u8]]) {
        let Some(dst_addr) = self.dst_addr else {
            return;
        };
        
        let mut packet_buf = Vec::new();

        let full_len = data.iter().fold(0, |acc, x| acc + x.len());
        let full_data = data.iter().fold(Vec::new(), |mut acc, x| { acc.extend_from_slice(x); acc });

        let mut size = 0;
        trace!("UDP packet chunks: {}", full_len.div_ceil(1300));
        for (chunk_num, chunk) in full_data.chunks(SHORT_PACKET_SIZE).enumerate() {
            // 1) Packet type pattern
            packet_buf.extend_from_slice(&packet_type.pattern());

            // 2) Seq id
            let seq_id = self.new_seq_num();
            let seq_id_bytes = seq_id.to_be_bytes();
            packet_buf.extend_from_slice(&seq_id_bytes);

            // 3) Flags
            let mut flags = PacketFlags::empty();
            if size + chunk.len() == full_len {
                flags.insert(PacketFlags::PacketEnd);
            }
            if chunk_num == 0 {
                flags.insert(PacketFlags::PacketStart);
            }
            if full_len <= SHORT_PACKET_SIZE {
                flags.insert(PacketFlags::ShortPacket);
            }
            packet_buf.push(flags.as_u8());
            
            if !flags.contains(PacketFlags::ShortPacket) {
                // 4) chunk num
                packet_buf.extend_from_slice(&[chunk_num as u8]);
            }

            // 5) Data
            packet_buf.extend_from_slice(chunk);
            
            if let Err(e) = self.socket.send_to(&packet_buf, dst_addr) {
                warn!("Error sending packet to client: {e:?}");
                return;
            }
            packet_buf.clear();
            size += chunk.len();
            
            // Throttle sending to roughly 60MB/s
            if size % 10_000 > 10_000 - SHORT_PACKET_SIZE {
                thread::sleep(Duration::from_micros(100));
            }
        }
        // Special case 
        if data.is_empty() {
            let mut packet_buf = Vec::new();
            packet_buf.extend_from_slice(&packet_type.pattern());
            let seq_id = self.new_seq_num();
            let seq_id_bytes = seq_id.to_be_bytes();
            packet_buf.extend_from_slice(&seq_id_bytes);
            let flags = PacketFlags::PacketStart | PacketFlags::PacketEnd | PacketFlags::ShortPacket;
            packet_buf.push(flags.as_u8());
            if let Err(e) = self.socket.send_to(&packet_buf, dst_addr) {
                warn!("Error sending packet to client: {e:?}");
            }
        }
    }
    fn with_timestamp_freq_request(mut self, timestamp_freq_request: Arc<AtomicBool>) -> Self
    where
        Self: Sized,
    {
        self.timestamp_freq_request = timestamp_freq_request;
        self
    }
    fn poll(&mut self) {
        self.try_recv();
        if let Some(discovery_socket) = self.discovery_socket.as_mut() {
            discovery_socket.poll(|_|{});
        }
    }
}

impl ConfiguredSender for UdpSender {
    type Config = UdpSenderConfig;
    fn new(cfg: &Self::Config) -> Option<Self> {
        let desired_port = cfg.desired_port.unwrap_or_default();
        let socket = match UdpSocket::bind(("0.0.0.0", desired_port)) {
            Ok(socket) => {
                let local_port = socket.local_addr().unwrap().port();
                info!("[sparkles] Udp socket bound to port {local_port}");
                socket
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                let res = UdpSocket::bind("0.0.0.0:0")
                    .inspect_err(|e| error!("Unable to bind UDP socket: {e:?}"))
                    .ok()?;
                let local_port = res.local_addr().unwrap().port();
                warn!("Unable to bind to specific port {desired_port}, using random port {local_port}");
                res

            }
            Err(e) => {
                error!("Error binding UDP socket: {e:?}");
                return None;
            }
        };
        
        socket.set_nonblocking(true).ok()?;

        let discovery_socket = cfg.multicast_discovery_config.as_ref().map(|cfg| {
            let mut res = MulticastDiscoverySocket::new_with_service(cfg, socket.local_addr().unwrap().port(), ()).unwrap();
            res.set_discover_replies_en(true);
            res
        });

        Some(Self {
            socket,
            discovery_socket,
            dst_addr: None,
            seq_num: 1,
            last_recv: None,
            timestamp_freq_request: Arc::new(AtomicBool::new(false)),
        })
    }
}