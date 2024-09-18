mod perfetto_format;

use std::cmp::min;
use std::collections::BTreeMap;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use sparkles_core::headers::{LocalPacketHeader, ThreadNameHeader};
use sparkles_core::local_storage::id_mapping::EventType;
use crate::perfetto_format::PerfettoTraceFile;

#[derive(Default)]
pub struct TraceAcceptor {
    event_parsers: BTreeMap<u64, ThreadParserState>,
    total_event_bytes: u64,
    total_transport_bytes: u64,
}

#[derive(Default)]
pub struct ThreadParserState {
    thread_name: String,
    event_buf: Vec<(LocalPacketHeader, Vec<TracingEvent>)>,

    cur_started_ranges: BTreeMap<u8, (TracingEventId, u64)>,
    state_machine: ParsingStateMachine,
    cur_tm: u64,
}


lazy_static! {
    pub static ref TRACE_RESULT_FILE: Mutex<PerfettoTraceFile> = Mutex::new(PerfettoTraceFile::default());
}

impl TraceAcceptor {
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
                        match event {
                            TracingEvent::Instant(_, dif_tm) => {
                                parser_state.cur_tm += dif_tm;
                            }
                            TracingEvent::RangePart(_, dif_tm, _) => {
                                  parser_state.cur_tm += dif_tm;
                            }
                            TracingEvent::UnnamedRangeEnd(dif_tm, _) => {
                                parser_state.cur_tm += dif_tm;
                            }
                        }
                    }
                    // add to trace file
                    let timestamp = (parser_state.cur_tm as f64 / header.counts_per_ns) as u64;
                    match event {
                        TracingEvent::Instant(id, _) => {
                            let (ev_name, _) = &header.id_store.tags[*id as usize];
                            trace_res_file.add_point_event(format!("{}.{}", thread_name, ev_name), thread_id, timestamp);
                        }
                        TracingEvent::RangePart(id, _, ord_id) => {
                            let (ev_name, ev_type) = &header.id_store.tags[*id as usize];
                            if let EventType::RangeEnd(start_id) = ev_type {
                                let (start_name, _) = &header.id_store.tags[*start_id as usize];
                                let start_info = parser_state.cur_started_ranges.remove(ord_id).unwrap();
                                let start_tm = start_info.1;
                                let duration = timestamp - start_tm;
                                trace_res_file.add_range_event(format!("{}.(Range){} -> {}", thread_name, start_name, ev_name), thread_id, start_tm, duration);
                            }
                            else {
                                // Range start
                                parser_state.cur_started_ranges.insert(*ord_id, (*id, timestamp));
                            }
                        }
                        TracingEvent::UnnamedRangeEnd(_, ord_id ) => {
                            let start_info = parser_state.cur_started_ranges.remove(ord_id).unwrap();
                            let range_name = start_info.0;
                            let start_tm = start_info.1;
                            let duration = timestamp - start_tm;
                            trace_res_file.add_range_event(format!("{}.(Range){}", thread_name, range_name), thread_id, start_tm, duration);
                        }
                    }
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

                        // TODO: if comment out something unexplainable happens...
                        let mut trace_res_file = TRACE_RESULT_FILE.lock().unwrap();
                        let timestamp = (header.start_timestamp as f64 / header.counts_per_ns) as u64;
                        let duration = ((header.end_timestamp - header.start_timestamp) as f64 / header.counts_per_ns) as u64;
                        trace_res_file.add_range_event("Local packet".to_string(), header.thread_id as usize, timestamp, duration);

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
                    let duration = ((header.end_timestamp - header.start_timestamp) as f64 / header.counts_per_ns) as u64;
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
pub enum TracingEvent {
    Instant(TracingEventId, u64),
    RangePart(TracingEventId, u64, u8),
    UnnamedRangeEnd(u64, u8)
}

#[derive(Copy,Clone, Default)]
pub enum ParsingStateMachine {
    #[default]
    NewFrame,
    DifPrLen(TracingEventId),

    /// id, dif_tm, left_dif_tm_bytes
    DifTm(TracingEventId, u64, u8),

    RangeOrdId(Option<TracingEventId>, u8),
    RangeTm(Option<TracingEventId>, u8, u64, u8)
}

impl ParsingStateMachine {
    pub fn next_byte(&mut self, b: u8) -> Option<TracingEvent> {
        match *self {
            ParsingStateMachine::NewFrame => {
                *self = ParsingStateMachine::DifPrLen(b);
                None
            }
            ParsingStateMachine::DifPrLen(ev) => {
                let is_range_event = b & 0b1000_0000 != 0;
                let is_unnamed_range_end = b & 0b0100_0000 != 0;
                let dif_tm_len = b & 0b0000_1111;
                if is_range_event {
                    if is_unnamed_range_end {
                        *self = ParsingStateMachine::RangeOrdId(None, dif_tm_len);
                    }
                    else {
                        *self = ParsingStateMachine::RangeOrdId(Some(ev), dif_tm_len);
                    }
                    return None;
                }
                if dif_tm_len == 0 {
                    *self = ParsingStateMachine::NewFrame;
                    return Some(TracingEvent::Instant(ev, 0));
                }
                *self = ParsingStateMachine::DifTm(ev, 0, dif_tm_len);
                None
            }
            ParsingStateMachine::DifTm(ev, cur_dif_tm, left_bytes) => {
                let new_dif_tm = cur_dif_tm | ((b as u64) << ((left_bytes - 1) * 8));
                if left_bytes == 1 {
                    *self = ParsingStateMachine::NewFrame;
                    Some(TracingEvent::Instant(ev, new_dif_tm))
                }
                else {
                    *self= ParsingStateMachine::DifTm(ev, new_dif_tm, left_bytes - 1);
                    None
                }
            }
            ParsingStateMachine::RangeOrdId(ev, dif_tm_len) => {
                let ord_id = b;
                if dif_tm_len == 0 {
                    *self = ParsingStateMachine::NewFrame;
                    if let Some(id) = ev {
                        Some(TracingEvent::RangePart(id, 0, ord_id))
                    }
                    else {
                        Some(TracingEvent::UnnamedRangeEnd(0, ord_id))
                    }
                }
                else {
                    *self = ParsingStateMachine::RangeTm(ev, ord_id, 0, dif_tm_len);
                    None
                }
            }
            ParsingStateMachine::RangeTm(ev_id, ord_id, cur_dif_tm, left_bytes) => {
                let new_dif_tm = cur_dif_tm | ((b as u64) << ((left_bytes - 1) * 8));
                if left_bytes == 1 {
                    *self = ParsingStateMachine::NewFrame;
                    if let Some(id) = ev_id {
                        Some(TracingEvent::RangePart(id, new_dif_tm, ord_id))
                    }
                    else {
                        Some(TracingEvent::UnnamedRangeEnd(new_dif_tm, ord_id))
                    }
                }
                else {
                    *self = ParsingStateMachine::RangeTm(ev_id, ord_id, new_dif_tm, left_bytes - 1);
                    None
                }
            }
        }
    }

    pub fn parse_many(&mut self, bytes: &[u8]) -> Vec<TracingEvent> {
        bytes.iter().flat_map(|b| self.next_byte(*b)).collect()
    }
}