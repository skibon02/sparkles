mod perfetto_format;

use std::cmp::min;
use std::collections::BTreeMap;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use lazy_static::lazy_static;
use log::{debug, error, info};
use sparkles_core::headers::{LocalPacketHeader, ThreadNameHeader};
use crate::perfetto_format::PerfettoTraceFile;

pub struct TraceAcceptor {
    event_parsers: BTreeMap<u64, ThreadParserState>,
    total_event_bytes: u64,
    total_transport_bytes: u64,
}

#[derive(Default)]
pub struct ThreadParserState {
    thread_name: String,
    event_buf: Vec<(LocalPacketHeader, Vec<TracingEvent>)>,

    state_machine: ParsingStateMachine,
    cur_tm: u64,
}


lazy_static! {
    pub static ref TRACE_RESULT_FILE: Mutex<PerfettoTraceFile> = Mutex::new(PerfettoTraceFile::new());
}

impl TraceAcceptor {
    pub fn new() -> Self {
        Self {
            event_parsers: BTreeMap::new(),
            total_event_bytes: 0,
            total_transport_bytes: 0,
        }
    }

    pub fn listen(&mut self) -> Result<(), std::io::Error> {
        let listener = TcpListener::bind("0.0.0.0:4302")?;

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

        let mut trace_res_file = TRACE_RESULT_FILE.lock().unwrap();

        let mut counts_per_ns = 1.0;
        // iterate over all threads
        for (&thread_id, parser_state) in &mut self.event_parsers {
            let thread_id = thread_id as usize;
            let thread_name = &parser_state.thread_name;
            // iterate over events
            for (header, events) in &parser_state.event_buf {
                counts_per_ns = header.counts_per_ns;

                trace_res_file.set_thread_name(thread_id, thread_name.clone());
                // for (id, tag) in header.id_store.id_map.iter().enumerate() {
                // }

                parser_state.cur_tm = header.start_timestamp;
                let mut first = true;
                for event in events {
                    if first {
                        first = false;
                    }
                    else {
                        parser_state.cur_tm += event.1;
                    }
                    // add to trace file
                    let timestamp = (parser_state.cur_tm as f64 / header.counts_per_ns) as u64;
                    let ev_name = &header.id_store.id_map[event.0 as usize];
                    trace_res_file.add_point_event(format!("{}.{}", thread_name, ev_name), thread_id, timestamp);
                }
                total_events += events.len();
                if header.start_timestamp < min_timestamp {
                    min_timestamp = header.start_timestamp;
                }
                if header.end_timestamp > max_timestamp {
                    max_timestamp = header.end_timestamp;
                }
                covered_dur += header.end_timestamp - header.start_timestamp;

            }
        }

        let events_per_sec = total_events as f64 / ((max_timestamp - min_timestamp) as f64 / counts_per_ns) * 1_000_000_000.0;
        let events_per_sec_covered = total_events as f64 / (covered_dur as f64 / counts_per_ns) * 1_000_000_000.0;
        info!("Total events: {}", total_events);
        info!("Events per second (global): {} eps", events_per_sec);
        info!("Events per second (covered): {} eps", events_per_sec_covered);
        info!("Average event duration: {} ns", covered_dur as f64 / counts_per_ns / total_events as f64);
        info!("Average bytes per event: {} bytes", self.total_event_bytes as f64 / total_events as f64);
        info!("Average transport bytes per event: {} bytes", self.total_transport_bytes as f64 / total_events as f64);

        info!("Finished!");

        Ok(())
    }

    fn handle_client(&mut self, con: &mut TcpStream) -> Result<(), std::io::Error> {
        loop {
            let mut packet_type = [0u8; 1];
            con.read_exact(&mut packet_type)?;
            info!("Packet id: {}", packet_type[0]);

            let mut events_bytes = vec![0; 10_000];

            match packet_type[0] {
                0x01 => {
                    let mut total_bytes = [0u8; 8];
                    con.read_exact(&mut total_bytes)?;
                    let mut total_bytes = u64::from_le_bytes(total_bytes) as usize;

                    while total_bytes > 0 {
                        let mut header_len = [0u8; 8];
                        con.read_exact(&mut header_len)?;
                        self.total_transport_bytes += 8;
                        let header_len = u64::from_le_bytes(header_len) as usize;

                        let mut header_bytes = vec![0u8; header_len];
                        con.read_exact(&mut header_bytes)?;
                        self.total_transport_bytes += header_len as u64;
                        let header = bincode::deserialize::<LocalPacketHeader>(&header_bytes).unwrap();

                        let mut buf_len = [0u8; 8];
                        con.read_exact(&mut buf_len)?;
                        self.total_transport_bytes += 8;
                        let buf_len = u64::from_le_bytes(buf_len) as usize;

                        let mut event_buf = Vec::with_capacity(10_000);
                        info!("Got packet header: {:?}", header);

                        let thread_id = header.thread_ord_id;
                        let cur_parser_state = self.event_parsers.entry(thread_id).or_default();

                        // let mut trace_res_file = TRACE_RESULT_FILE.lock().unwrap();
                        // let timestamp = (header.initial_timestamp as f64 / header.counts_per_ns) as u64;
                        // let duration = ((header.end_timestamp - header.initial_timestamp) as f64 / header.counts_per_ns) as u32;
                        // trace_res_file.add_range_event(format!("Local packet #{}", packet_num), header.thread_id as usize, 0, timestamp, duration);

                        let mut remaining_size = buf_len;
                        while remaining_size > 0 {
                            let cur_size = min(1_000_000, remaining_size);
                            events_bytes.resize(cur_size, 0);
                            con.read_exact(&mut events_bytes)?;
                            self.total_transport_bytes += cur_size as u64;

                            let new_events = cur_parser_state.state_machine.parse_many(&events_bytes);
                            let new_events_len = new_events.len();
                            event_buf.extend_from_slice(&new_events);
                            debug!("Got {} bytes, Parsed {} events", cur_size, new_events_len);
                            self.total_event_bytes += cur_size as u64;

                            remaining_size -= cur_size;
                        }

                        total_bytes -= 8 + 8 + header_len + buf_len;

                        cur_parser_state.event_buf.push((header, event_buf));
                    }
                },
                0x02 => {
                    let mut header_len = [0u8; 8];
                    con.read_exact(&mut header_len)?;
                    self.total_transport_bytes += 8;
                    let header_len = u64::from_le_bytes(header_len) as usize;

                    let mut header_bytes = vec![0u8; header_len];
                    con.read_exact(&mut header_bytes)?;
                    self.total_transport_bytes += header_len as u64;
                    let header = bincode::deserialize::<LocalPacketHeader>(&header_bytes).unwrap();

                    info!("Got failed packet header: {:?}", header);

                    let mut trace_res_file = TRACE_RESULT_FILE.lock().unwrap();
                    let timestamp = (header.start_timestamp as f64 / header.counts_per_ns) as u64;
                    let duration = ((header.end_timestamp - header.start_timestamp) as f64 / header.counts_per_ns) as u32;
                    trace_res_file.add_range_event("Missed events page".to_string(), header.thread_ord_id as usize, timestamp, duration);

                },
                0x03 => {
                    let mut header_len = [0u8; 8];
                    con.read_exact(&mut header_len)?;
                    self.total_transport_bytes += 8;
                    let header_len = u64::from_le_bytes(header_len) as usize;

                    let mut header_bytes = vec![0u8; header_len];
                    con.read_exact(&mut header_bytes)?;
                    self.total_transport_bytes += header_len as u64;
                    let header = bincode::deserialize::<ThreadNameHeader>(&header_bytes).unwrap();

                    info!("Got thread name: {:?}", header);

                    let thread_id = header.thread_ord_id;
                    let cur_parser_state = self.event_parsers.entry(thread_id).or_default();

                    cur_parser_state.thread_name = header.thread_name;
                },
                0xff => {
                    info!("Client was gracefully disconnected!");

                    return Ok(());
                }
                _ => panic!("Unknown packet type!")
            }
        }
    }
}

pub type TracingEventId = u8;

/// event, dif_tm
#[derive(Debug, Copy, Clone)]
pub struct TracingEvent(TracingEventId, u64);

#[derive(Copy,Clone, Default)]
pub enum ParsingStateMachine {
    #[default]
    NewFrame,
    DifPrLen(TracingEventId),

    /// id, dif_tm, left_dif_tm_bytes
    DifTm(TracingEventId, u64, u8)
}

impl ParsingStateMachine {
    pub fn next_byte(&mut self, b: u8) -> Option<TracingEvent> {
        match *self {
            ParsingStateMachine::NewFrame => {
                *self = ParsingStateMachine::DifPrLen(b);
                None
            }
            ParsingStateMachine::DifPrLen(ev) => {
                let dif_tm_len = b & 0b0000_1111;
                if dif_tm_len == 0 {
                    *self = ParsingStateMachine::NewFrame;
                    return Some(TracingEvent(ev, 0));
                }
                *self = ParsingStateMachine::DifTm(ev, 0, dif_tm_len);
                None
            }
            ParsingStateMachine::DifTm(ev, cur_dif_tm, left_bytes) => {
                let new_dif_tm = cur_dif_tm | ((b as u64) << ((left_bytes - 1) * 8));
                if left_bytes == 1 {
                    *self = ParsingStateMachine::NewFrame;
                    Some(TracingEvent(ev, new_dif_tm))
                }
                else {
                    *self= ParsingStateMachine::DifTm(ev, new_dif_tm, left_bytes - 1);
                    None
                }
            }
        }
    }

    pub fn parse_many(&mut self, bytes: &[u8]) -> Vec<TracingEvent> {
        bytes.iter().flat_map(|b| self.next_byte(*b)).collect()
    }
}