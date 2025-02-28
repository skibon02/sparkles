use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use log::{info, warn};
use sparkles_core::protocol::packets::{PacketType, RequestPacketType};
use sparkles_core::protocol::sender::{ConfiguredSender, PacketFlags, Sender};


static SOMEONE_CONNECTED: AtomicBool = AtomicBool::new(false);
pub fn is_someone_connected() -> bool {
    SOMEONE_CONNECTED.swap(false, std::sync::atomic::Ordering::Relaxed)
}

pub(crate) struct UdpSender {
    socket: UdpSocket,
    dst_addr: Option<SocketAddr>,
    last_recv: Option<Instant>,
    seq_id: u16,
}
impl UdpSender {
    fn new_seq_id(&mut self) -> u16 {
        let seq_id = self.seq_id;
        self.seq_id = self.seq_id.wrapping_add(1);
        seq_id
    }
    
    fn try_recv(&mut self) {
        let mut buf = [0u8; 32];
        match self.socket.recv_from(&mut buf) {
            Ok((32, addr)) if buf == RequestPacketType::Subscribe.header() => {
                info!("[sparkles] UDP client connected: {addr:?}");
                self.dst_addr = Some(addr);
                SOMEONE_CONNECTED.store(true, std::sync::atomic::Ordering::Relaxed);
                self.last_recv = Some(Instant::now());
            }
            Ok(_) => {
                warn!("[sparkles] Incorrect packet received from client! Ignoring...");
                return;
            }
            Err(e) => {
                // Got nothing
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    return;
                }
                
                warn!("[sparkles] Error receiving packet from client: {}", e);
                return;
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
        for (chunk_num, chunk) in full_data.chunks(SHORT_PACKET_SIZE).enumerate() {
            // 1) Packet type pattern
            packet_buf.extend_from_slice(&packet_type.header());

            // 2) Seq id
            let seq_id = self.new_seq_id();
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
                let chunk_num_bytes = (chunk_num as u8).to_be_bytes();
                packet_buf.extend_from_slice(&chunk_num_bytes);
            }

            // 5) Data
            packet_buf.extend_from_slice(chunk);
            
            if let Err(e) = self.socket.send_to(&packet_buf, dst_addr) {
                warn!("Error sending packet to client: {}", e);
                return;
            }
            packet_buf.clear();
            size += chunk.len();
        }
    }
}

impl ConfiguredSender for UdpSender {
    type Config = UdpSenderConfig;
    fn new(cfg: &Self::Config) -> Option<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::new(127, 0, 0, 1), cfg.local_port.unwrap_or(38338))).ok()?;
        
        socket.set_nonblocking(true).ok()?;

        Some(Self {
            socket,
            dst_addr: None,
            seq_id: 0,
            last_recv: None,
        })
    }
}