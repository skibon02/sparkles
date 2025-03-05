use std::{io, thread};
use std::io::{BufRead, Read};
use std::net::{ToSocketAddrs, UdpSocket};
use std::time::Duration;
use enumset::EnumSet;
use log::{debug, info, warn};
use thiserror::Error;
use sparkles_core::protocol::headers::{LocalPacketHeader, SparklesMachineInfo};
use sparkles_core::protocol::packets::{PacketType, RequestPacketType};
use sparkles_core::protocol::sender::PacketFlags;

pub enum Packet {
    MachineInfo(SparklesMachineInfo),
    DataBytes(Vec<(LocalPacketHeader, Vec<u8>)>),
    FailedPages(Vec<LocalPacketHeader>),
    TimestampFreq(u64, u64),
    GracefulShutdown,
    ConnectionAccepted,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct ProtocolCounters {
    pub protocol_overhead: usize,
    pub trace_buf: usize,
    pub secondary_packets: usize,
}

impl ProtocolCounters {
    pub fn total_bytes(&self) -> usize {
        self.protocol_overhead + self.trace_buf + self.secondary_packets
    }
}

pub enum PacketDecoder {
    Stream{
        stream: Box<dyn BufRead>,
        is_eof: bool,
        counters: ProtocolCounters,
    },
    Socket{
        socket: UdpSocket,
        is_eof: bool,
        counters: ProtocolCounters,
        
        last_seq_num: u16,

        partial_packet_info: Option<UdpParserState>,
    }
}

pub struct UdpParserState {
    received_chunks: Vec<(usize, Vec<u8>)>,
    starting_num: usize,
}

impl UdpParserState {
    pub fn new(chunk_num: u8, data: &[u8]) -> Self {
        UdpParserState {
            received_chunks: vec![(chunk_num as usize, data.to_vec())],
            starting_num: 0,
        }
    }
    pub fn push(&mut self, chunk_num: u8, chunk: Vec<u8>) -> bool {
        if chunk_num == 0 && !self.received_chunks.is_empty() {
            self.starting_num += 256;
        }
        
        let chunk_num = chunk_num as usize + self.starting_num;
        if self.received_chunks.iter().any(|(num, _)| *num == chunk_num) {
            return false;
        }
        self.received_chunks.push((chunk_num, chunk));
        true
    }
    pub fn build(&mut self) -> Option<Vec<u8>> {
        let max_chunk_num = *self.received_chunks.iter().map(|(num, _)| num).max().unwrap();
        if max_chunk_num + 1 != self.received_chunks.len() {
            return None;
        }

        let mut res = Vec::new();
        self.received_chunks.sort_by_key(|(num, _)| *num);
        for (_, chunk) in &self.received_chunks {
            res.extend_from_slice(chunk);
        }
        Some(res)
    }
}


pub type ReadResult<T> = Result<T, PacketReadError>;
#[derive(Error, Debug)]
pub enum PacketReadError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Decode error: {0}")]
    DecodeError(#[from] bincode::error::DecodeError),

    #[error("End of file reached")]
    Eof,
    #[error("Packet length too big")]
    LengthTooBig,


    // Udp errors
    #[error("Udp packet too short")]
    UdpPacketTooShort,
    #[error("Incorrect packet type pattern")]
    IncorrectPattern,
    #[error("Out of order sequence number")]
    OutOfOrderSeqNum,
    #[error("Incomplete long packet")]
    IncompleteLongPacket,
}
impl PacketDecoder {
    pub fn from_stream(stream: impl Read + 'static) -> Self {
        let stream = Box::new(io::BufReader::new(stream));
        PacketDecoder::Stream{
            stream,
            is_eof: false,
            counters: ProtocolCounters::default(),
        }
    }

    pub fn from_socket(addr: impl ToSocketAddrs) -> Self {
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket.connect(addr).unwrap();

        loop {
            socket.send(&RequestPacketType::Subscribe.pattern()).unwrap();

            socket.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            let mut buf = [0u8; 32];
            match socket.recv(&mut buf) {
                Ok(32) if buf == PacketType::ConnectionAccepted.pattern() => {
                    break;
                }
                Ok(_) => {
                    warn!("Incorrect packet received from server! Ignoring...");
                    thread::sleep(Duration::from_millis(500));
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        continue;
                    }
                    if e.kind() != io::ErrorKind::ConnectionReset {
                        warn!("Error receiving packet from server: {}", e);
                    }
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }

        PacketDecoder::Socket{
            socket,
            is_eof: false,
            counters: ProtocolCounters::default(),
            partial_packet_info: None,
            last_seq_num: 0,
        }
    }

    pub fn read_packet(&mut self) -> ReadResult<Option<Packet>> {
        match self {
            PacketDecoder::Stream {
                stream, is_eof,
                counters
            } => {
                if *is_eof {
                    return Err(PacketReadError::Eof);
                }

                // read packet header and length
                let mut packet_type_buf = vec![0; 32];
                stream.read_exact(&mut packet_type_buf)?;
                counters.protocol_overhead += 32;
                
                let mut read_find_packet_start = || -> Result<PacketType, io::Error> {
                    loop {
                        if let Some(header) = PacketType::try_from_pattern(&packet_type_buf[..32]) {
                            return Ok(header);
                        }

                        // Shift buffer by 1 byte
                        packet_type_buf.remove(0);
                        let mut new_byte = [0; 1];
                        stream.read_exact(&mut new_byte)?;
                        counters.protocol_overhead += 32;
                        packet_type_buf.push(new_byte[0]);
                    }
                };

                let packet_type = read_find_packet_start();
                let Ok(packet_type) = packet_type else {
                    return Err(packet_type.unwrap_err().into());
                };
                let mut length_buf = [0; 4];
                stream.read_exact(&mut length_buf)?;
                counters.protocol_overhead += 4;
                let length = u32::from_be_bytes(length_buf);
                if length < 100_000_000 {
                    // Parse packet data
                    let mut data = vec![0; length as usize];
                    stream.read_exact(&mut data)?;

                    let res = parse_packet_from_data(packet_type, &data)?;
                    if matches!(res, Packet::GracefulShutdown) {
                        *is_eof = true;
                    }
                    if matches!(res, Packet::DataBytes(_)) {
                        counters.trace_buf += length as usize;
                    }
                    else {
                        counters.secondary_packets += length as usize;
                    }
                    Ok(Some(res))
                }
                else {
                    warn!("[PacketDecoder] Packet size too large! Ignoring...");
                    Err(PacketReadError::LengthTooBig)
                }
            }
            PacketDecoder::Socket{
                socket, is_eof,
                partial_packet_info,
                last_seq_num,
                counters
            } => unsafe {
                if *is_eof {
                    return Err(PacketReadError::Eof);
                }

                let mut buf = vec![0; 1400];
                let new_packet_sz = socket.recv(&mut buf)?;
                let packet = &buf[..new_packet_sz];
                if new_packet_sz < 32 + 3 {
                    warn!("[PacketDecoder] Udp packet too short! Ignoring...");
                    return Err(PacketReadError::UdpPacketTooShort);
                }
                let packet_type = &packet[..32];
                let Some(packet_type) = PacketType::try_from_pattern(packet_type) else {
                    warn!("[PacketDecoder] Udp packet type not recognized! Ignoring...");
                    return Err(PacketReadError::IncorrectPattern);
                };
                counters.protocol_overhead += 32;
                let seq_num = u16::from_be_bytes(packet[32..34].try_into().unwrap());
                counters.protocol_overhead += 2;

                if *last_seq_num < u16::MAX - 100 {
                    match seq_num.cmp(&(*last_seq_num)) {
                        std::cmp::Ordering::Less => {
                            warn!("[PacketDecoder] Udp packet sequence number out of order! Ignoring...");
                            Err(PacketReadError::OutOfOrderSeqNum)
                        }
                        std::cmp::Ordering::Equal => {
                            warn!("[PacketDecoder] Udp packet sequence number repeated! Ignoring...");
                            Err(PacketReadError::OutOfOrderSeqNum)
                        }
                        std::cmp::Ordering::Greater => {
                            if seq_num > *last_seq_num + 1 {
                                let lost_cnt = seq_num - *last_seq_num - 1;
                                warn!("[PacketDecoder] We lost {} packets!", lost_cnt);
                            }
                            *last_seq_num = seq_num;

                            let flags = EnumSet::from_repr_unchecked(packet[34]);
                            counters.protocol_overhead += 1;
                            let (res, data_len) = if flags.contains(PacketFlags::ShortPacket) {
                                let data = &packet[35..];
                                let data_len = data.len();

                                if partial_packet_info.is_some() {
                                    warn!("[PacketDecoder] Resetting partial packet info!");
                                    *partial_packet_info = None;
                                }
                                (Some(parse_packet_from_data(packet_type, data)?), data_len)
                            }
                            else {
                                let chunk_num = packet[35];
                                let data = &packet[36..];
                                let data_len = data.len();


                                if let Some(udp_state) = partial_packet_info {
                                    // info!("Long packet: chunk {chunk_num}, data_size: {data_len}");
                                    if !udp_state.push(chunk_num, data.to_vec()) {
                                        warn!("[PacketDecoder] Duplicate or incomplete chunks for long packet! Ignoring...");
                                        return Err(PacketReadError::IncompleteLongPacket);
                                    }
                                    if flags.contains(PacketFlags::PacketEnd) {
                                        info!("It was last chunk, building packet...");
                                        let long_packet_data = udp_state.build();
                                        *partial_packet_info = None;
                                        if let Some(data) = long_packet_data {
                                            (Some(parse_packet_from_data(packet_type, &data)?), data_len)
                                        }
                                        else {
                                            warn!("Some chunks of long packet are missing! Skipping...");
                                            return Err(PacketReadError::IncompleteLongPacket)
                                        }
                                    }
                                    else {
                                        (None, data_len)
                                    }
                                }
                                else {
                                    if !flags.contains(PacketFlags::PacketStart) {
                                        warn!("Assertion failed! Udp packet chunk without start flag!");
                                    }
                                    *partial_packet_info = Some(UdpParserState::new(chunk_num, data));
                                    (None, data_len)
                                }
                            };


                            if packet_type == PacketType::DataBytes {
                                counters.trace_buf += data_len;
                            }
                            else {
                                counters.secondary_packets += data_len;

                                if packet_type == PacketType::GracefulShutdown {
                                    *is_eof = true;
                                }
                            }
                            Ok(res)
                        }
                    }
                }
                else {
                    *last_seq_num = 0;
                    unimplemented!("Seq num wrap-around!");
                }
            }
        }
    }

    pub fn is_eof(&self) -> bool {
        match self {
            PacketDecoder::Socket{is_eof, ..} |
            PacketDecoder::Stream{is_eof, ..} => *is_eof
        }
    }
    pub fn counters(&self) -> ProtocolCounters {
        match self {
            PacketDecoder::Socket{counters, ..} |
            PacketDecoder::Stream{counters, ..} => *counters
        }
    }
}

fn parse_packet_from_data(packet_type: PacketType, data: &[u8]) -> ReadResult<Packet> {
    match packet_type {
        PacketType::GracefulShutdown => {
            Ok(Packet::GracefulShutdown)
        }
        PacketType::MachineInfo => {
            info!("Got MachineInfo packet!");
            let (machine_info, sz) = bincode::decode_from_slice(data, bincode_config())?;
            if sz != data.len() {
                warn!("[PacketDecoder] Assertion failed! MachineInfo packet size mismatch!");
            }
            Ok(Packet::MachineInfo(machine_info))
        }
        PacketType::DataBytes => {
            let mut res = Vec::new();
            let mut cursor = 0;
            loop {
                if cursor + 8 >= data.len() {
                    warn!("[PacketDecoder] DataBytes partial data received!");
                    break;
                }
                let length = u64::from_be_bytes(data[cursor..cursor + 8].try_into().unwrap()) as usize;
                cursor += 8;
                debug!("local packet header len: {length}");

                if cursor + length >= data.len() {
                    warn!("[PacketDecoder] DataBytes partial data received!");
                    break;
                }
                let (local_packet_header, sz) = bincode::decode_from_slice(&data[cursor..], bincode_config())?;
                if sz != length {
                    warn!("[PacketDecoder] Assertion failed! LocalPacketHeader packet size mismatch!");
                }
                cursor += length;
                
                debug!("LocalPacketHeader received!");

                if cursor + 8 >= data.len() {
                    warn!("[PacketDecoder] DataBytes partial data received!");
                    break;
                }
                let buf_len = u64::from_be_bytes(data[cursor..cursor + 8].try_into().unwrap()) as usize;
                cursor += 8;

                debug!("Data len received: {buf_len}");

                if cursor + buf_len > data.len() {
                    warn!("[PacketDecoder] DataBytes partial data received!");
                    break;
                }
                let buf = data[cursor..cursor + buf_len].to_vec();
                res.push((local_packet_header, buf));
                cursor += buf_len;
                debug!("Got {}KB of data!", buf_len as f32 / 1024.0);
                
                if cursor == data.len() {
                    break;
                }
            }
            Ok(Packet::DataBytes(res))
        }
        PacketType::FailedPages => {
            let (failed_pages, sz) = bincode::decode_from_slice(&data, bincode_config())?;
            if sz != data.len() {
                warn!("[PacketDecoder] Assertion failed! FailedPages packet size mismatch!");
            }
            Ok(Packet::FailedPages(failed_pages))
        }
        PacketType::TimestampFreq => {
            let freq = u64::from_be_bytes(data[..8].try_into().unwrap());
            let cur_tm = u64::from_be_bytes(data[8..16].try_into().unwrap());
            Ok(Packet::TimestampFreq(freq, cur_tm))
        }
        PacketType::ConnectionAccepted => {
            Ok(Packet::ConnectionAccepted)
        }
    }
}

fn bincode_config() -> impl bincode::config::Config {
    bincode::config::standard().with_limit::<100_000>()
}