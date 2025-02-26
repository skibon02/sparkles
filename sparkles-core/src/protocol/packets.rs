use alloc::vec::Vec;
use core::array::TryFromSliceError;
use bincode::error::DecodeError;
use crate::protocol::headers::{LocalPacketHeader, SparklesMachineInfo};
use crate::protocol::sender::Sender;

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum PacketType {
    MachineInfo,
    DataBytes,
    FailedPages,
    TimestampFreq,
    GracefulShutdown,
}

impl PacketType {
    pub const fn get_str(&self) -> &str {
        match self {
            PacketType::MachineInfo => "MachineInfo",
            PacketType::DataBytes => "DataBytes",
            PacketType::FailedPages => "FailedPages",
            PacketType::TimestampFreq => "TimestampFreq",
            PacketType::GracefulShutdown => "GracefulShutdown"
        }
    }
    pub const fn header(&self) -> [u8; 32] {
        let str = self.get_str();
        sha2_const::Sha256::new().update(str.as_bytes()).finalize()
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
    pub const fn header(&self) -> [u8; 32] {
        let str = self.get_str();
        sha2_const::Sha256::new().update(str.as_bytes()).finalize()
    }
}
pub fn send_machine_info(sender: &mut impl Sender, sparkles_encoder_info: SparklesMachineInfo) {
    let encoded_info = bincode::encode_to_vec(&sparkles_encoder_info, bincode::config::standard()).unwrap();
    sender.send_packet(PacketType::MachineInfo, &[&encoded_info]);
}
pub fn parse_machine_info(data: &[u8]) -> Result<(SparklesMachineInfo, usize), DecodeError> {
    bincode::decode_from_slice(data, bincode::config::standard())
}

pub fn send_trace_data(sender: &mut impl Sender, slice1: &[u8], slice2: &[u8]) {
    sender.send_packet(PacketType::DataBytes, &[slice1, slice2]);
}

pub fn send_failed_pages(sender: &mut impl Sender, failed_pages: &[LocalPacketHeader]) {
    let header = bincode::encode_to_vec(failed_pages, bincode::config::standard()).unwrap();
    sender.send_packet(PacketType::FailedPages, &[&header]);
}
pub fn parse_failed_pages(data: &[u8]) -> Result<(Vec<LocalPacketHeader>, usize), DecodeError> {
    bincode::decode_from_slice(data, bincode::config::standard())
}

pub fn send_timestamp_freq(sender: &mut impl Sender, ticks_per_sec: u64) {
    let bytes = ticks_per_sec.to_le_bytes();
    sender.send_packet(PacketType::TimestampFreq, &[&bytes]);
}
pub fn parse_timestamp_freq(data: &[u8]) -> Result<u64, TryFromSliceError> {
    let data = data.try_into()?;
    Ok(u64::from_le_bytes(data))
}

pub fn send_graceful_shutdown(sender: &mut impl Sender) {
    sender.send_packet(PacketType::GracefulShutdown, &[]);
}