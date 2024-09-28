use std::net::{Ipv4Addr, UdpSocket};
use sparkles_core::sender::{ConfiguredSender, Sender};

pub(crate) struct UdpSender {
    socket: UdpSocket,
    dst_addr: Option<(Ipv4Addr, u16)>
}

#[derive(Debug, Default, Clone)]
pub struct UdpSenderConfig {
    pub local_port: Option<u16>
}
impl Sender for UdpSender {
    fn send(&mut self, data: &[u8]) {
        if let Some(addr) = self.dst_addr {
            self.socket.send_to(data, addr).unwrap();
        }
    }
}

impl ConfiguredSender for UdpSender {
    type Config = UdpSenderConfig;
    fn new(cfg: &Self::Config) -> Option<Self> {
        let res = if let Some(port) = cfg.local_port {
            let socket = UdpSocket::bind((Ipv4Addr::new(127, 0, 0, 1), port)).ok()?;

            Self {
                socket,
                dst_addr: None
            }
        }
        else {
            let socket = UdpSocket::bind((Ipv4Addr::new(127, 0, 0, 1), 38338)).ok()?;

            Self {
                socket,
                dst_addr: None
            }
        };

        Some(res)
    }
}