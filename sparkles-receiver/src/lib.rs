mod perfetto_format;

use std::{mem, thread};
use std::cmp::min;
use std::collections::BTreeMap;
use std::io::Read;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use lazy_static::lazy_static;
use log::{debug, error, info};
use sparkles::LocalPacketHeader;
use crate::perfetto_format::PerfettoTraceFile;

pub struct TraceAcceptor {
    stream_parser: ParsingStateMachine,
}

const MEASURE_DUR_NS: usize = 19;

lazy_static! {
    pub static ref TRACE_RESULT_FILE: Mutex<PerfettoTraceFile> = Mutex::new(PerfettoTraceFile::new());
}

impl TraceAcceptor {
    pub fn new() -> Self {
        Self {
            stream_parser: ParsingStateMachine::EventId
        }
    }

    pub fn listen(&mut self) -> Result<(), std::io::Error> {
        let listener = TcpListener::bind("0.0.0.0:4302").unwrap();

        let mut total_pr = 0;
        let mut first = true;

        info!("Server running at port 4302");
        info!("Waiting for connection...");
        let mut buf = vec![0; 1_000_000];
        // let mut reader = BufReader::with_capacity(5_000_000, con);

        let mut bytes_cnt = 0;
        let mut events_cnt = 0;

        let mut threads_info = BTreeMap::new();

        let mut events = BTreeMap::new();
        for con in listener.incoming().take(1) {
            if let Ok(mut con) = con {
                info!("Client connected!");
                if let Err(e) = self.handle_client(&mut con, &mut events, &mut threads_info) {
                    error!("Error handling client: {:?}", e);
                    break;
                }
            }
            else {
                error!("Error accepting connection: {:?}", con);
            }
        }

        info!("Disconnected... Start parsing");

        let mut id_offset = 0;
        let mut trace_res_file = TRACE_RESULT_FILE.lock().unwrap();
        for (thread_id, header) in threads_info {
            for (id, tag) in header.id_store.id_map.iter().enumerate() {
                trace_res_file.set_thread_name(id + id_offset, header.thread_name.clone() + "." + tag);
            }

            for event in events.get(&thread_id).unwrap() {
                if first {
                    first = false;
                }
                else {
                    total_pr += event.2;
                }
                // add to trace file
                trace_res_file.add_point_event(format!("{:?}", event.0), event.0 as usize + id_offset, ((event.1 as u64 | (total_pr << 16)) as f64 / 2.495) as u64);
            }
            id_offset += 256;
        }

        info!("Total PR: {}", total_pr);

        info!("Finished!");

        Ok(())
    }

    fn handle_client(&mut self, con: &mut TcpStream, events: &mut BTreeMap<u64, Vec<TracingEvent>>, threads_info: &mut BTreeMap<u64, LocalPacketHeader>) -> Result<(), std::io::Error> {
        loop {
            let mut packet_type = [0u8; 1];
            con.read_exact(&mut packet_type)?;
            info!("Packet id: {}", packet_type[0]);

            match packet_type[0] {
                0x01 => {
                    let mut total_bytes = [0u8; 8];
                    con.read_exact(&mut total_bytes)?;

                    let mut total_bytes = usize::from_be_bytes(total_bytes);
                    while total_bytes > 0 {

                        let mut header_len = [0u8; 8];
                        con.read_exact(&mut header_len)?;
                        let header_len = usize::from_be_bytes(header_len);
                        let mut header_bytes = vec![0u8; header_len];
                        con.read_exact(&mut header_bytes)?;
                        let header = bincode::deserialize::<LocalPacketHeader>(&header_bytes).unwrap();

                        info!("Got packet header: {:?}", header);
                        let thread_id = header.thread_id;
                        threads_info.insert(thread_id, header.clone());



                        let mut remaining_size = header.buf_length;
                        while remaining_size > 0 {
                            let cur_size = min(1_000_000, remaining_size);
                            let mut cur_buf = vec![0; cur_size];
                            con.read_exact(&mut cur_buf)?;

                            let new_events = self.stream_parser.parse_many(&cur_buf, &header);
                            let new_events_len = new_events.len();
                            events.entry(thread_id).or_insert_with(|| Vec::with_capacity(1_000_000)).extend(new_events);
                            debug!("Got {} bytes, Parsed {} events", cur_size, new_events_len);

                            remaining_size -= cur_size;
                        }

                        total_bytes -= 8 + header_len + header.buf_length;
                    }
                },
                0x02 => {

                    let mut header_len = [0u8; 8];
                    con.read_exact(&mut header_len)?;
                    let mut header_bytes = vec![0u8; usize::from_be_bytes(header_len)];
                    con.read_exact(&mut header_bytes)?;
                    let header = bincode::deserialize::<LocalPacketHeader>(&header_bytes).unwrap();

                    info!("Got failed packet header: {:?}", header);
                },
                _ => panic!("Unknown packet type!")
            }
        }
    }
}

pub type TracingEventId = u8;

/// event, timestamp end (cpu cycles), dif_pr (24 bits)
#[derive(Debug, Copy, Clone)]
pub struct TracingEvent(TracingEventId, u16, u64);

#[derive(Copy,Clone, Default)]
pub enum ParsingStateMachine {
    #[default]
    EventId,
    TimestampHigh(TracingEventId),
    TimestampLow(TracingEventId, u8),
    TimestampPrOrEventId(TracingEventId, u16, u64, u8),
}

impl ParsingStateMachine {
    pub fn next_byte(&mut self, b: u8) -> Option<TracingEvent> {
        match *self {
            ParsingStateMachine::EventId => unsafe {
                *self = ParsingStateMachine::TimestampHigh(TracingEventId::from(b & 0x7F));
                None
            }
            ParsingStateMachine::TimestampHigh(event_id) => {
                *self = ParsingStateMachine::TimestampLow(event_id, b);
                None
            }
            ParsingStateMachine::TimestampLow(event_id, now) => {
                *self = ParsingStateMachine::TimestampPrOrEventId(event_id, ((now as u16) << 8) | b as u16, 0, 0);
                None
            }
            ParsingStateMachine::TimestampPrOrEventId(event_id, now, pr, cur_shift) => unsafe {
                if b & 0x80 != 0 {
                    // new event start, finalize current event
                    *self = ParsingStateMachine::TimestampHigh(TracingEventId::from(b & 0x7F));
                    Some(TracingEvent(event_id, now, pr))
                }
                else {
                    *self = ParsingStateMachine::TimestampPrOrEventId(event_id, now, ((b as u64) << cur_shift as u64) | pr, cur_shift + 7);
                    None
                }
            }
        }
    }

    pub fn parse_many(&mut self, bytes: &[u8], header: &LocalPacketHeader) -> Vec<TracingEvent> {
        bytes.iter().flat_map(|b| self.next_byte(*b)).collect()
    }
}