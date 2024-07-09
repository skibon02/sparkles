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
    event_parsers: BTreeMap<u64, ThreadParserState>,
}

#[derive(Default)]
pub struct ThreadParserState {
    event_buf: Vec<(LocalPacketHeader, Vec<TracingEvent>)>,

    state_machine: ParsingStateMachine,
    cur_pr: u64,
}

const MEASURE_DUR_NS: usize = 19;

lazy_static! {
    pub static ref TRACE_RESULT_FILE: Mutex<PerfettoTraceFile> = Mutex::new(PerfettoTraceFile::new());
}

impl TraceAcceptor {
    pub fn new() -> Self {
        Self {
            event_parsers: BTreeMap::new()
        }
    }

    pub fn listen(&mut self) -> Result<(), std::io::Error> {
        let listener = TcpListener::bind("0.0.0.0:4302").unwrap();

        info!("Server running at port 4302");
        info!("Waiting for connection...");

        for con in listener.incoming().take(1) {
            if let Ok(mut con) = con {
                info!("Client connected!");
                if let Err(e) = self.handle_client(&mut con) {
                    error!("Error handling client: {:?}", e);
                    break;
                }
            }
            else {
                error!("Error accepting connection: {:?}", con);
            }
        }

        info!("Disconnected... Start parsing");

        //some stats
        let mut total_events = 0;
        let mut min_timestamp = u64::MAX;
        let mut max_timestamp = 0;
        let mut covered_dur = 0;

        let mut id_offset = 0;
        let mut trace_res_file = TRACE_RESULT_FILE.lock().unwrap();
        // iterate over all threads
        for (_thread_id, mut parser_state) in &mut self.event_parsers {
            // iterate over events
            for (header, events) in &parser_state.event_buf {
                for (id, tag) in header.id_store.id_map.iter().enumerate() {
                    trace_res_file.set_thread_name(id + id_offset, header.thread_name.clone() + "." + tag);
                }

                parser_state.cur_pr = header.initial_timestamp >> 16;
                let mut first = true;
                for event in events {
                    if first {
                        first = false;
                    }
                    else {
                        parser_state.cur_pr += event.2;
                    }
                    // add to trace file
                    let timestamp = ((event.1 as u64 | (parser_state.cur_pr << 16)) as f64 / 2.495) as u64;
                    trace_res_file.add_point_event(format!("{:?}", header.thread_name), event.0 as usize + id_offset, timestamp);
                }
                total_events += events.len();
                if header.initial_timestamp < min_timestamp {
                    min_timestamp = header.initial_timestamp;
                }
                if header.end_timestamp > max_timestamp {
                    max_timestamp = header.end_timestamp;
                }
                covered_dur += header.end_timestamp - header.initial_timestamp;

            }
            id_offset += 256;
        }

        let events_per_sec = total_events as f64 / ((max_timestamp - min_timestamp) as f64 / 2.495) * 1_000_000_000.0;
        let events_per_sec_covered = total_events as f64 / (covered_dur as f64 / 2.495) * 1_000_000_000.0;
        info!("Total events: {}", total_events);
        info!("Events per second (global): {} eps", events_per_sec);
        info!("Events per second (covered): {} eps", events_per_sec_covered);

        info!("Finished!");

        Ok(())
    }

    fn handle_client(&mut self, con: &mut TcpStream) -> Result<(), std::io::Error> {
        loop {
            let mut packet_type = [0u8; 1];
            con.read_exact(&mut packet_type)?;
            info!("Packet id: {}", packet_type[0]);

            let mut packet_num = 0;
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

                        let mut event_buf = Vec::with_capacity(10_000);
                        info!("Got packet header: {:?}", header);
                        let thread_id = header.thread_id;

                        let mut cur_parser_state = self.event_parsers.entry(thread_id).or_default();

                        let mut trace_res_file = TRACE_RESULT_FILE.lock().unwrap();
                        let timestamp = ((header.initial_timestamp as f64 / 2.495) as u64) as u64;
                        let duration = ((header.end_timestamp - header.initial_timestamp) as f64 / 2.495) as u32;
                        trace_res_file.add_range_event(format!("Local packet #{}", packet_num), 666 + header.thread_id as usize, timestamp, duration);


                        let mut remaining_size = header.buf_length;
                        while remaining_size > 0 {
                            let cur_size = min(1_000_000, remaining_size);
                            let mut cur_buf = vec![0; cur_size];
                            con.read_exact(&mut cur_buf)?;

                            let new_events = cur_parser_state.state_machine.parse_many(&cur_buf);
                            let new_events_len = new_events.len();
                            event_buf.extend(new_events);
                            debug!("Got {} bytes, Parsed {} events", cur_size, new_events_len);

                            remaining_size -= cur_size;
                        }

                        total_bytes -= 8 + header_len + header.buf_length;
                        packet_num += 1;

                        cur_parser_state.event_buf.push((header, event_buf));
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

    pub fn parse_many(&mut self, bytes: &[u8]) -> Vec<TracingEvent> {
        bytes.iter().flat_map(|b| self.next_byte(*b)).collect()
    }
}