use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::{Duration, Instant};
use log::{debug, info, warn};
use sparkles_core::protocol::packets::{PacketType, RequestPacketType};
use sparkles_core::protocol::sender::{ConfiguredSender, PacketFlags, Sender};
use crate::on_client_connect;

static SOMEONE_CONNECTED: AtomicBool = AtomicBool::new(false);
pub fn is_someone_connected() -> bool {
    SOMEONE_CONNECTED.swap(false, std::sync::atomic::Ordering::Relaxed)
}

pub(crate) struct UdpSender {
    socket: UdpSocket,
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
                info!("[sparkles] UDP client connected: {addr:?}");
                self.dst_addr = Some(addr);
                SOMEONE_CONNECTED.store(true, std::sync::atomic::Ordering::Relaxed);
                self.last_recv = Some(Instant::now());
                self.timestamp_freq_request.store(true, std::sync::atomic::Ordering::Relaxed);
                on_client_connect();
                
                self.socket.connect(addr).unwrap();
                if let Err(e) = self.socket.send(&PacketType::ConnectionAccepted.pattern()) {
                    warn!("[sparkles] Error sending ConnectionAccepted packet to client: {}", e);
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
                
                warn!("[sparkles] Error receiving packet from client: {}", e);
            }
        }
    }
}

const SHORT_PACKET_SIZE: usize = 1300;
#[derive(Debug, Default, Clone)]
pub struct UdpSenderConfig {
    pub local_port: Option<u16>
}

impl Sender for UdpSender {
    fn send_packet(&mut self, packet_type: PacketType, data: &[&[u8]]) {
        if self.dst_addr.is_none() || self.last_recv.is_none_or(|i| i.elapsed().as_secs() > 5) {
            self.try_recv();
        }
        
        let Some(dst_addr) = self.dst_addr else {
            return;
        };
        
        let mut packet_buf = Vec::new();

        let full_len = data.iter().fold(0, |acc, x| acc + x.len());
        let full_data = data.iter().fold(Vec::new(), |mut acc, x| { acc.extend_from_slice(x); acc });

        let mut size = 0;
        debug!("UDP packet chunks: {}", full_len.div_ceil(1300));
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
            
            if let Err(e) = self.socket.send(&packet_buf) {
                warn!("Error sending packet to client: {}", e);
                return;
            }
            packet_buf.clear();
            size += chunk.len();
            
            // Throttle sending
            // if size % 100_000 > 100_000 - SHORT_PACKET_SIZE {
            //     thread::sleep(Duration::from_micros(100));
            // }
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
            if let Err(e) = self.socket.send(&packet_buf) {
                warn!("Error sending packet to client: {}", e);;
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
}

impl ConfiguredSender for UdpSender {
    type Config = UdpSenderConfig;
    fn new(cfg: &Self::Config) -> Option<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", cfg.local_port.unwrap_or(38338))).ok()?;
        
        socket.set_nonblocking(true).ok()?;

        Some(Self {
            socket,
            dst_addr: None,
            seq_num: 1,
            last_recv: None,
            timestamp_freq_request: Arc::new(AtomicBool::new(false)),
        })
    }
}