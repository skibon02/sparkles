use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bincode::{Decode, Encode};
use sha2_const_stable::Sha256;
use crate::protocol::headers::{LocalPacketHeader, SparklesMachineInfo};
use crate::protocol::sender::Sender;

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum PacketType {
    MachineInfo,
    DataBytes,
    FailedPages,
    SyncPoint,
    GracefulShutdown,
    ConnectionAccepted,
    ExternalEvents,
    ExternalEventNames,
    ExternalSyncPoint
}

impl PacketType {
    /// This string is used to calculate hash of the packet type
    pub const fn get_str(&self) -> &str {
        match self {
            PacketType::MachineInfo => "MachineInfo",
            PacketType::DataBytes => "DataBytes",
            PacketType::FailedPages => "FailedPages",
            PacketType::SyncPoint => "SyncPoint",
            PacketType::GracefulShutdown => "GracefulShutdown",
            PacketType::ConnectionAccepted => "ConnectionAccepted",
            PacketType::ExternalEvents => "ExternalEvents",
            PacketType::ExternalEventNames => "ExternalEventNames",
            PacketType::ExternalSyncPoint => "ExternalSyncPoint",
        }
    }
    pub const fn pattern(&self) -> [u8; 32] {
        let str = self.get_str();
        Sha256::new().update(str.as_bytes()).finalize()
    }
    
    pub fn try_from_pattern(pattern: &[u8]) -> Option<Self> {
        if pattern == Self::MachineInfo.pattern() {
            Some(Self::MachineInfo)
        }
        else if pattern == Self::DataBytes.pattern() {
            Some(Self::DataBytes)
        }
        else if pattern == Self::FailedPages.pattern() {
            Some(Self::FailedPages)
        }
        else if pattern == Self::SyncPoint.pattern() {
            Some(Self::SyncPoint)
        }
        else if pattern == Self::GracefulShutdown.pattern() {
            Some(Self::GracefulShutdown)
        }
        else if pattern == Self::ConnectionAccepted.pattern() {
            Some(Self::ConnectionAccepted)
        }
        else if pattern == Self::ExternalEvents.pattern() {
            Some(Self::ExternalEvents)
        }
        else if pattern == Self::ExternalEventNames.pattern() {
            Some(Self::ExternalEventNames)
        }
        else if pattern == Self::ExternalSyncPoint.pattern() {
            Some(Self::ExternalSyncPoint)
        }
        else {
            None
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum RequestPacketType {
    Subscribe,
}
impl RequestPacketType {
    pub const fn get_str(&self) -> &str {
        match self {
            RequestPacketType::Subscribe => "Subscribe",
        }
    }
    pub const fn pattern(&self) -> [u8; 32] {
        let str = self.get_str();
        Sha256::new().update(str.as_bytes()).finalize()
    }
}
pub fn send_machine_info(sender: &mut impl Sender, sparkles_encoder_info: SparklesMachineInfo) {
    let encoded_info = bincode::encode_to_vec(&sparkles_encoder_info, bincode::config::standard()).unwrap();
    sender.send_packet(PacketType::MachineInfo, &[&encoded_info]);
}
pub fn send_trace_data(sender: &mut impl Sender, slice1: &[u8], slice2: &[u8]) {
    sender.send_packet(PacketType::DataBytes, &[slice1, slice2]);
}

pub fn send_failed_pages(sender: &mut impl Sender, failed_pages: &[LocalPacketHeader]) {
    let header = bincode::encode_to_vec(failed_pages, bincode::config::standard()).unwrap();
    sender.send_packet(PacketType::FailedPages, &[&header]);
}
pub fn send_sync_point(sender: &mut impl Sender, monotonic_tm: u64, cur_tm: u64) {
    let monotonic_tm_bytes = monotonic_tm.to_be_bytes();
    let tm_bytes = cur_tm.to_be_bytes();
    sender.send_packet(PacketType::SyncPoint, &[&monotonic_tm_bytes, &tm_bytes]);
}
pub fn send_graceful_shutdown(sender: &mut impl Sender) {
    sender.send_packet(PacketType::GracefulShutdown, &[]);
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct ExternalEvents {
    pub ext_ord_id: u32,
    pub start_timestamp: u64,
    pub bytes_per_timestamp: u8,
}


pub fn send_external_events(sender: &mut impl Sender, header: ExternalEvents, data: &[u8]) {
    let encoded_header = bincode::encode_to_vec(&header, bincode::config::standard()).unwrap();
    sender.send_packet(PacketType::ExternalEvents, &[&encoded_header, data]);
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct ExternalEventNames {
    pub ext_ord_id: u32,
    pub channel_name: Arc<str>,
    pub event_names: Vec<String>,
}

pub fn send_external_event_names(sender: &mut impl Sender, header: ExternalEventNames) {
    let encoded_header = bincode::encode_to_vec(&header, bincode::config::standard()).unwrap();
    sender.send_packet(PacketType::ExternalEventNames, &[&encoded_header]);
}

pub fn send_external_sync_point(sender: &mut impl Sender, ext_ord_id: u32, local_timestamp: u64,
external_timestamp: u64) {
    let ext_ord_id_bytes = ext_ord_id.to_be_bytes();
    let local_bytes = local_timestamp.to_be_bytes();
    let external_bytes = external_timestamp.to_be_bytes();
    sender.send_packet(PacketType::ExternalSyncPoint, &[&ext_ord_id_bytes, &local_bytes, &external_bytes]);
}