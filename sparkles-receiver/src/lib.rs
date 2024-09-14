mod perfetto_format;

use std::cmp::min;
use std::collections::BTreeMap;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
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
            // iterate over events
            for (header, events) in &parser_state.event_buf {
                counts_per_ns = header.counts_per_ns;

                trace_res_file.set_thread_name(thread_id as usize % 65590, header.thread_name.clone());
                // for (id, tag) in header.id_store.id_map.iter().enumerate() {
                // }

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
                    let timestamp = ((event.1 as u64 | (parser_state.cur_pr << 16)) as f64 / header.counts_per_ns) as u64;
                    let ev_name = &header.id_store.id_map[event.0 as usize];
                    trace_res_file.add_point_event(format!("{}.{}", &header.thread_name, ev_name), thread_id as usize % 65590, timestamp);
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
        }

        let events_per_sec = total_events as f64 / ((max_timestamp - min_timestamp) as f64 / counts_per_ns) * 1_000_000_000.0;
        let events_per_sec_covered = total_events as f64 / (covered_dur as f64 / counts_per_ns) * 1_000_000_000.0;
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

                        let cur_parser_state = self.event_parsers.entry(thread_id).or_default();

                        // let mut trace_res_file = TRACE_RESULT_FILE.lock().unwrap();
                        // let timestamp = (header.initial_timestamp as f64 / header.counts_per_ns) as u64;
                        // let duration = ((header.end_timestamp - header.initial_timestamp) as f64 / header.counts_per_ns) as u32;
                        // trace_res_file.add_range_event(format!("Local packet #{}", packet_num), header.thread_id as usize, 0, timestamp, duration);


                        let mut remaining_size = header.buf_length;
                        while remaining_size > 0 {
                            let cur_size = min(1_000_000, remaining_size);
                            let mut cur_buf = vec![0; cur_size as usize];
                            con.read_exact(&mut cur_buf)?;

                            let new_events = cur_parser_state.state_machine.parse_many(&cur_buf);
                            let new_events_len = new_events.len();
                            event_buf.extend(new_events);
                            debug!("Got {} bytes, Parsed {} events", cur_size, new_events_len);

                            remaining_size -= cur_size;
                        }

                        total_bytes -= 8 + header_len + header.buf_length as usize;
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

/// event, now (16 bits), dif_pr (48 bits)
#[derive(Debug, Copy, Clone)]
pub struct TracingEvent(TracingEventId, u16, u64);

#[derive(Copy,Clone, Default)]
pub enum ParsingStateMachine {
    #[default]
    NewFrame,
    DifPrLen(TracingEventId),
    Now(TracingEventId, u8),
    Now2(TracingEventId, u8, u16),

    /// Id, now, dif_pr, left_dif_pr
    DifPr(TracingEventId, u16, u64, u8)
}

impl ParsingStateMachine {
    pub fn next_byte(&mut self, b: u8) -> Option<TracingEvent> {
        match *self {
            ParsingStateMachine::NewFrame => {
                *self = ParsingStateMachine::DifPrLen(b);
                None
            }
            ParsingStateMachine::DifPrLen(ev) => {
                // let have_emb_data = b & 0b1000 != 0;
                // if have_emb_data {
                //     unimplemented!("Have embedded data!");
                // }
                *self = ParsingStateMachine::Now(ev, b);
                None
            }
            ParsingStateMachine::Now(ev, dif_pr_len) => {
                *self = ParsingStateMachine::Now2(ev, dif_pr_len, b as u16);
                None
            }
            ParsingStateMachine::Now2(ev, dif_pr_len, cur_now) => {
                let new_cur_now = cur_now | (b as u16) << 8;
                if dif_pr_len == 0 {
                    *self = ParsingStateMachine::NewFrame;
                    Some(TracingEvent(ev, new_cur_now, 0))
                }
                else {
                    *self = ParsingStateMachine::DifPr(ev, new_cur_now, 0, dif_pr_len);
                    None
                }
            }
            ParsingStateMachine::DifPr(ev, now, cur_dif_pr, left_bytes) => {
                let new_dif_pr = cur_dif_pr | ((b as u64) << ((left_bytes - 1) * 8));
                if left_bytes == 1 {
                    *self = ParsingStateMachine::NewFrame;
                    Some(TracingEvent(ev, now, new_dif_pr))
                }
                else {
                    *self= ParsingStateMachine::DifPr(ev, now, new_dif_pr, left_bytes - 1);
                    None
                }
            }
        }
    }

    pub fn parse_many(&mut self, bytes: &[u8]) -> Vec<TracingEvent> {
        bytes.iter().flat_map(|b| self.next_byte(*b)).collect()
    }
}