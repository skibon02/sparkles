use std::io;
use std::io::{BufRead, Read};
use std::net::{SocketAddr, UdpSocket};
use enumset::__internal::EnumSetTypePrivate;
use enumset::EnumSet;
use log::warn;
use thiserror::Error;
use sparkles_core::protocol::headers::{LocalPacketHeader, SparklesMachineInfo};
use sparkles_core::protocol::packets::PacketType;
use sparkles_core::protocol::sender::PacketFlags;

pub enum Packet {
    MachineInfo(SparklesMachineInfo),
    DataBytes(Vec<(LocalPacketHeader, Vec<u8>)>),
    FailedPages(Vec<LocalPacketHeader>),
    TimestampFreq(u64),
    GracefulShutdown,
}
pub enum PacketDecoder {
    Stream{
        stream: Box<dyn BufRead>,
        is_eof: bool,
    },
    Socket{
        socket: UdpSocket,
        is_eof: bool,
        
        last_seq_num: u16,

        partial_packet_info: Option<UdpParserState>,
    }
}

pub struct UdpParserState {

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
}
impl PacketDecoder {
    pub fn from_stream(stream: impl Read + 'static) -> Self {
        let stream = Box::new(io::BufReader::new(stream));
        PacketDecoder::Stream{
            stream,
            is_eof: false,
        }
    }

    pub fn from_socket(addr: SocketAddr) -> Self {
        let socket = UdpSocket::bind("0.0.0.0").unwrap();
        socket.connect(addr).unwrap();

        PacketDecoder::Socket{
            socket,
            is_eof: false,
            partial_packet_info: None,
            last_seq_num: 0,
        }
    }

    pub fn read_packet(&mut self) -> ReadResult<Packet> {
        match self {
            PacketDecoder::Stream {
                stream, is_eof
            } => {
                if *is_eof {
                    return Err(PacketReadError::Eof);
                }

                // read packet header and length
                let mut packet_type_buf = vec![0; 32];
                stream.read_exact(&mut packet_type_buf)?;
                
                let mut read_find_packet_start = || -> Result<PacketType, io::Error> {
                    loop {
                        if let Some(header) = PacketType::try_from_pattern(&packet_type_buf[..32]) {
                            return Ok(header);
                        }

                        // Shift buffer by 1 byte
                        packet_type_buf.remove(0);
                        let mut new_byte = [0; 1];
                        stream.read_exact(&mut new_byte)?;
                        packet_type_buf.push(new_byte[0]);
                    }
                };

                let packet_type = read_find_packet_start();
                let Ok(packet_type) = packet_type else {
                    return Err(packet_type.unwrap_err().into());
                };
                let mut length_buf = [0; 4];
                stream.read_exact(&mut length_buf)?;
                let length = u32::from_be_bytes(length_buf);
                if length < 4_000_000 {
                    // Parse packet data
                    let mut data = vec![0; length as usize];
                    stream.read_exact(&mut data)?;

                    let res = parse_packet_from_data(packet_type, &data)?;
                    if matches!(res, Packet::GracefulShutdown) {
                        *is_eof = true;
                    }
                    Ok(res)
                }
                else {
                    warn!("[PacketDecoder] Packet size too large! Ignoring...");
                    Err(PacketReadError::LengthTooBig)
                }
            }
            PacketDecoder::Socket{
                socket, is_eof,
                partial_packet_info,
                last_seq_num
            } => unsafe {
                if *is_eof {
                    return Err(PacketReadError::Eof);
                }

                let mut buf = vec![0; 1400];
                let new_packet_sz = socket.recv(&mut buf)?;
                let packet = &buf[..new_packet_sz];
                if new_packet_sz < 32 + 4 {
                    warn!("[PacketDecoder] Udp packet too short! Ignoring...");
                    return Err(PacketReadError::UdpPacketTooShort);
                }
                let packet_type = &packet[..32];
                let Some(packet_type) = PacketType::try_from_pattern(packet_type) else {
                    warn!("[PacketDecoder] Udp packet type not recognized! Ignoring...");
                    return Err(PacketReadError::IncorrectPattern);
                };
                let seq_num = u16::from_be_bytes(packet[32..34].try_into().unwrap());
                
                if *last_seq_num < u16::MAX - 100 {
                    if seq_num < *last_seq_num {
                        warn!("[PacketDecoder] Udp packet sequence number out of order! Ignoring...");
                        return Err(PacketReadError::OutOfOrderSeqNum);
                    }
                    else if seq_num == *last_seq_num {
                        warn!("[PacketDecoder] Udp packet sequence number repeated! Ignoring...");
                        return Err(PacketReadError::OutOfOrderSeqNum);
                    }
                    else {
                        *last_seq_num = seq_num;
                        if seq_num > *last_seq_num + 1 {
                            warn!("[PacketDecoder] We lost some packets!");
                        }
                        
                        let flags = EnumSet::from_repr_unchecked(buf[34]);
                        if flags.contains(PacketFlags::ShortPacket) {
                            let data = &buf[35..];
                            let res = parse_packet_from_data(packet_type, data)?;
                            if matches!(res, Packet::GracefulShutdown) {
                                *is_eof = true;
                            }
                            return Ok(res);
                        }
                        else {
                            unimplemented!("Got a long packet!");
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
    
}

fn parse_packet_from_data(packet_type: PacketType, data: &[u8]) -> ReadResult<Packet> {
    match packet_type {
        PacketType::GracefulShutdown => {
            Ok(Packet::GracefulShutdown)
        }
        PacketType::MachineInfo => {
            let (machine_info, sz) = bincode::decode_from_slice(&data, bincode_config())?;
            if sz != data.len() {
                warn!("[PacketDecoder] Assertion failed! MachineInfo packet size mismatch!");
            }
            Ok(Packet::MachineInfo(machine_info))
        }
        PacketType::DataBytes => {
            let mut res = Vec::new();
            let mut cursor = 0;
            loop {
                cursor += 8;
                if cursor >= data.len() {
                    warn!("[PacketDecoder] DataBytes partial data received!");
                    break;
                }
                let length = u64::from_be_bytes(data[cursor..cursor + 8].try_into().unwrap());

                cursor += length as usize;
                if cursor >= data.len() {
                    warn!("[PacketDecoder] DataBytes partial data received!");
                    break;
                }
                let (local_packet_header, sz) = bincode::decode_from_slice(&data[cursor..], bincode_config())?;

                cursor += 8;
                if cursor >= data.len() {
                    warn!("[PacketDecoder] DataBytes partial data received!");
                    break;
                }
                let buf_len = u64::from_be_bytes(data[cursor..cursor + 8].try_into().unwrap());
                let buf = data[cursor..cursor + buf_len as usize].to_vec();
                res.push((local_packet_header, buf));

                cursor += buf_len as usize;
                if cursor >= data.len() {
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
            Ok(Packet::TimestampFreq(freq))
        }
    }
}

fn bincode_config() -> impl bincode::config::Config {
    bincode::config::standard().with_limit::<100_000>()
}