use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::sync::atomic::{AtomicBool, Ordering};
use enumset::{EnumSet, EnumSetType};
use crate::protocol::packets::PacketType;

/// Abstraction for the destination of captured events
///
/// After putting events into the global storage,
/// multiple senders can be used to transfer events to remote client or long-term storage.
pub trait Sender {
    fn send_packet(&mut self, packet_type: PacketType, data: &[&[u8]]);
    fn with_timestamp_freq_request(self, timestamp_freq_request: Arc<AtomicBool>) -> Self
    where
        Self: Sized;
    
    fn poll(&mut self) {}
}

pub trait ConfiguredSender: Sender + Sized {
    type Config: Debug;
    fn new(cfg: &Self::Config) -> Option<Self>;
    fn new_default() -> Option<Self>
    where
        Self::Config: Default
    {
        Self::new(&Self::Config::default())
    }
}

/// Storage for multiple senders, which is also a sender.
#[derive(Default)]
pub struct SenderChain {
    senders: Vec<Box<dyn Sender>>,
    timestamp_freq_request: Arc<AtomicBool>,
}

impl SenderChain {
    pub fn take_tm_freq_requested(&self) -> bool {
        self.timestamp_freq_request.swap(false, Ordering::Relaxed)
    }
}

impl SenderChain {
    pub fn with_sender<T: Sender + 'static>(&mut self, sender: T) {
        self.senders.push(Box::new(sender.with_timestamp_freq_request(self.timestamp_freq_request.clone())));
    }
}

impl Sender for SenderChain {
    fn send_packet(&mut self, packet_type: PacketType, data: &[&[u8]]) {
        for sender in self.senders.iter_mut() {
            sender.send_packet(packet_type, data);
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
        for sender in self.senders.iter_mut() {
            sender.poll();
        }
    }
}

#[derive(Debug, EnumSetType)]
#[enumset(repr="u8")]
pub enum PacketFlags {
    ShortPacket,
    PacketEnd,
    PacketStart
}

impl PacketFlags {
    pub fn empty() -> EnumSet<PacketFlags> {
        EnumSet::empty()
    }
}