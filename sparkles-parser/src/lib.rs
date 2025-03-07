mod perfetto_format;
mod consts;
pub mod tracing_decoder;
pub mod parsed;
pub mod packet_decoder;

use std::collections::BTreeMap;
use std::io::Read;
use std::net::{ToSocketAddrs};
use std::ops::Deref;
use std::rc::Rc;
use std::{mem, thread};
use std::time::Duration;
use bytes::BytesMut;
use log::{debug, error, info, warn};
use sparkles_core::local_storage::id_mapping::EventType;
use sparkles_core::protocol::headers::SparklesMachineInfo;
use crate::packet_decoder::{Packet, PacketDecoder, PacketReadError};
use crate::parsed::{ParsedEvent, ThreadInfoState};
use crate::tracing_decoder::StreamFrameDecoder;
use crate::perfetto_format::PerfettoTraceFile;

pub static PARSER_BUF_SIZE: usize = 1_000_000;

pub struct SparklesParser {
    packet_decoder: PacketDecoder,
    machine_info: Option<SparklesMachineInfo>,

    event_parsers: BTreeMap<u64, ThreadParserState>,
    local_packet_ranges: Vec<(usize, usize, u64, u64, u64)>,
    global_i: usize,
    interpolation_points: InterpolationPoints,
}

struct InterpolationPoints(BTreeMap<u64, (f64, u64)>); // key: cpu tm, value: (ticks_per_ns, timestamp nanos)

impl InterpolationPoints {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
    
    fn add_interpolation_point(&mut self, ticks_per_ns: f64, cur_tm: u64) {
        let ns = self.project_tm(cur_tm);
        self.0.insert(cur_tm, (ticks_per_ns, ns));
    }
    fn get_avg_ticks_per_ns(&self) -> f64 {
        self.0.values().map(|v| v.0).sum::<f64>() / self.0.len() as f64
    }
    
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn project_tm(&self, tm: u64) -> u64 {
        let inter_points = &self.0;
        let closest_left = inter_points.range(..=tm).next_back();
        let closest_right = inter_points.range(tm..).next();
        if let Some((left_tm, (left_slope, left_ns))) = closest_left {
            // interpolate using left slope
            let left_ns = *left_ns as f64;

            let slope = *left_slope;

            (left_ns + ((tm - *left_tm) as f64) / slope) as u64
        }
        else if let Some((right_tm, (right_slope, right_ns))) = closest_right {
            // interpolate using right slope
            let right_ns = *right_ns as f64;

            let slope = *right_slope;

            (right_ns - ((*right_tm - tm) as f64) / slope) as u64
        }
        else {
            tm
        }
    }
}

#[derive(Default)]
pub struct ThreadParserState {
    thread_name: Option<String>,
    thread_id: Option<u64>,
    last_thread_ord_id: u64,

    // start timestamp and duration for missed events packet
    missed_events: Vec<(u64, u64)>,

    // ---- TMP DATA ----
    state_machine: StreamFrameDecoder,
    // Helper for ranges handling
    cur_started_ranges: BTreeMap<u8, (TracingEventId, u64)>,
    // Current timestamp, accumulated from events
    cur_tm: u64,
    zero_diff_cnt: u64,
    
    id_store: BTreeMap<TracingEventId, (Rc<str>, EventType)>,
    stats: TracingStats,
}

#[derive(Copy, Clone, Default)]
pub struct TracingStats {
    pub total_events: usize,
    pub min_timestamp: u64,
    pub max_timestamp: u64,
    pub covered_dur: u64,
}

impl TracingStats {
    pub fn new_events(&mut self, events_cnt: usize, start_timestamp: u64, end_timestamp: u64) {
        self.total_events += events_cnt;
        if start_timestamp < self.min_timestamp {
            self.min_timestamp = start_timestamp;
        }
        if end_timestamp > self.max_timestamp {
            self.max_timestamp = end_timestamp;
        }
        self.covered_dur += end_timestamp - start_timestamp;
    }
}

impl ThreadParserState {
    pub fn thread_info_state(&self) -> ThreadInfoState {
        ThreadInfoState {
            thread_id: self.thread_id,
            thread_name: self.thread_name.clone(),
            thread_ord_id: self.last_thread_ord_id,
        }
    }
}

pub type ParseResult<T> = Result<T, PacketReadError>;

impl SparklesParser {
    /// Initialize parser from byte stream
    pub fn from_decoder(decoder: PacketDecoder) -> Self {
        Self {
            packet_decoder: decoder,

            machine_info: None,
            event_parsers: BTreeMap::new(),

            local_packet_ranges: Vec::new(),
            global_i: 0,
            interpolation_points: InterpolationPoints::new(),
        }
    }

    pub fn from_stream(stream: impl Read + 'static) -> Self {
        let decoder = PacketDecoder::from_stream(stream);
        Self::from_decoder(decoder)
    }

    pub fn from_udp_addr(addr: impl ToSocketAddrs) -> Self {
        let decoder = PacketDecoder::from_socket(addr);
        Self::from_decoder(decoder)
    }

    pub fn is_eof(&self) -> bool {
        self.packet_decoder.is_eof()
    }


    pub fn parse_to_end(&mut self, mut f: impl FnMut(&ParsedEvent, &ThreadInfoState)) -> ParseResult<()> {
        info!("Waiting for encoder info...");
        loop {
            match self.packet_decoder.read_packet() {
                Ok(Some(packet)) => {
                    match packet {
                        Packet::MachineInfo(info) => {
                            if info.ver != consts::ENCODER_VERSION {
                                warn!("Encoder version mismatch! Parser: {}, Encoder: {}", consts::ENCODER_VERSION, info.ver);
                            }

                            self.machine_info = Some(info);
                        }
                        Packet::TimestampFreq(ticks_per_sec, cur_tm) => {
                            let ticks_per_ns = ticks_per_sec as f64 / 1_000_000_000.0;
                            info!("Got timestamp frequency: {:?} t/ns", ticks_per_ns);

                            self.interpolation_points.add_interpolation_point(ticks_per_ns, cur_tm);
                        }
                        Packet::DataBytes(packets) => {
                            let global_i = self.global_i;
                            self.global_i += 1;
                            for (local_i, (header, data)) in packets.into_iter().enumerate() {
                                let thread_id = header.thread_ord_id;
                                let parser_state = self.event_parsers.entry(thread_id).or_default();

                                if self.interpolation_points.is_empty() {
                                    error!("Timestamp frequency is not set! Using default...");
                                }
                                self.local_packet_ranges.push((global_i, local_i, thread_id, self.interpolation_points.project_tm(header.start_timestamp), self.interpolation_points.project_tm(header.end_timestamp)));

                                //update thread name
                                if let Some(thread_info) = &header.thread_info {
                                    if let Some(thread_name) = thread_info.new_thread_name.clone() {
                                        parser_state.thread_name = Some(thread_name);
                                        parser_state.thread_id = Some(thread_info.thread_id);
                                    }
                                }
                                parser_state.last_thread_ord_id = thread_id;

                                let new_events = parser_state.state_machine.decode_many(&data);
                                let new_events_len = new_events.len();
                                debug!("Received {} events", new_events_len);

                                // Merge id store
                                for (id, (name, r#type)) in header.id_store.tags.iter().enumerate() {
                                    let id = id as u8;
                                    if let Some((old_name, old_type)) = parser_state.id_store.get(&id) {
                                        if old_name.as_ref() != name.deref() || old_type != r#type {
                                            error!("ID store mismatch for thread {:?}#{:?}! ID: {}, Old: {:?}, New: {:?}", parser_state.thread_name, parser_state.thread_id,
                                                id, (old_name, old_type), (name, r#type));
                                        }
                                    }
                                    parser_state.id_store.insert(id, (Rc::from(name.deref()), *r#type));
                                }
                                
                                parser_state.cur_tm = header.start_timestamp;
                                let mut first = true;
                                for evt in new_events {
                                    let mut dif_tm_zero = false;
                                    if first {
                                        first = false;
                                    }
                                    else {
                                        let dif_tm = match evt {
                                            TracingEvent::Instant(_, dif_tm) => dif_tm,
                                            TracingEvent::RangePart(_, dif_tm, _) => dif_tm,
                                            TracingEvent::UnnamedRangeEnd(dif_tm, _) => dif_tm
                                        };
                                        if dif_tm == 0 {
                                            dif_tm_zero = true;
                                        }
                                        parser_state.cur_tm += dif_tm;
                                    }
                                    if !dif_tm_zero {
                                        parser_state.zero_diff_cnt = 0;
                                    }
                                    else {
                                        parser_state.zero_diff_cnt += 1;
                                    }
                                    if parser_state.cur_tm > header.end_timestamp {
                                        warn!("Parsing issue: Timestamp is outside local packet! diff: {}",  parser_state.cur_tm - header.end_timestamp);
                                    }

                                    // Create ParsedEvent
                                    let timestamp = self.interpolation_points.project_tm(parser_state.cur_tm) + parser_state.zero_diff_cnt * 10;
                                    match evt {
                                        TracingEvent::Instant(id, _) => {
                                            let ev_name = if let Some((ev_name, ev_type)) = parser_state.id_store.get(&id) {
                                                if ev_type != &EventType::Instant {
                                                    error!("Assertion failed: Instant event type is not Instant!");
                                                }
                                                ev_name.clone()
                                            }
                                            else {
                                                error!("Did not find event name for id: {}", id);
                                                Rc::from(format!("Unknown Instant {}", id))
                                            };
                                            let parsed = ParsedEvent::Instant {
                                                name: ev_name.clone(),
                                                tm: timestamp
                                            };
                                            f(&parsed, &parser_state.thread_info_state());
                                        }
                                        TracingEvent::RangePart(id, _, ord_id) => {
                                            if let Some((ev_name, ev_type)) = parser_state.id_store.get(&id) {
                                                if ev_type == &EventType::Instant {
                                                    error!("Assertion failed: RangePart event has Instant type!");
                                                }
                                                else if let EventType::RangeEnd(start_id) = ev_type {
                                                    if let Some((start_name, start_ev_type)) = parser_state.id_store.get(start_id) {
                                                        if *start_ev_type != EventType::RangeStart {
                                                            error!("Assertion failed: RangePart event has wrong RangeStart type!");
                                                        }
                                                        if let Some((ev_id, start_tm)) = parser_state.cur_started_ranges.remove(&ord_id) {
                                                            if ev_id != *start_id {
                                                                error!("Assertion failed: RangePart event has wrong RangeEnd id!");
                                                            }
                                                            let parsed = ParsedEvent::NamedRange {
                                                                name: start_name.clone(),
                                                                end_name: ev_name.clone(),
                                                                start: start_tm,
                                                                end: timestamp
                                                            };
                                                            f(&parsed, &parser_state.thread_info_state());
                                                        }
                                                        else {
                                                            warn!("Did not find start event for RangePart id: {}", id);
                                                        }
                                                    }
                                                    else {
                                                        warn!("Did not find start event for RangePart id: {}", id);
                                                    }
                                                }
                                                else {
                                                    // Range start
                                                    parser_state.cur_started_ranges.insert(ord_id, (id, timestamp));
                                                }
                                            }
                                            else {
                                                error!("Did not find event name for id: {}", id);
                                                let ev_name: Rc<str> = Rc::from(format!("Unknown RangePart {}", id));

                                                let parsed = ParsedEvent::Instant {
                                                    name: ev_name,
                                                    tm: timestamp
                                                };
                                                f(&parsed, &parser_state.thread_info_state());
                                            };
                                            
                                        }
                                        TracingEvent::UnnamedRangeEnd(_, ord_id ) => {
                                            if let Some(start_info) = parser_state.cur_started_ranges.remove(&ord_id) {
                                                if let Some((start_name, ev_type)) = parser_state.id_store.get(&start_info.0) {
                                                    if *ev_type != EventType::RangeStart {
                                                        error!("Assertion failed: UnnamedRangeEnd event has non-RangeStart type!");
                                                    }
                                                    let parsed = ParsedEvent::Range {
                                                        name: start_name.clone(),
                                                        start: start_info.1,
                                                        end: timestamp
                                                    };
                                                    f(&parsed, &parser_state.thread_info_state());
                                                }
                                                else {
                                                    warn!("Did not find start event for UnnamedRangeEnd id: {}. Skipping...", ord_id);
                                                }
                                            }
                                            else {
                                                warn!("Did not find start event for UnnamedRangeEnd id: {}. Skipping...", ord_id);
                                            }
                                        }
                                    }
                                }
                                
                                parser_state.stats.new_events(new_events_len, header.start_timestamp, header.end_timestamp);
                                
                                parser_state.state_machine.ensure_buf_end();
                            }
                        }

                        Packet::FailedPages(failed_pages) => {
                            for header in failed_pages {
                                info!("Got failed pages header: {:?}", header);

                                let start = header.start_timestamp;
                                let dur = header.end_timestamp - header.start_timestamp;
                                let thread_ord_id = header.thread_ord_id;
                                self.thread_parser_state(thread_ord_id).missed_events.push((start, dur));
                            }
                        }
                        Packet::GracefulShutdown => {
                            info!("GracefulShutdown received!");
                            break;
                        }
                        Packet::ConnectionAccepted => {
                        }
                    }
                }
                Ok(None) => {
                    
                }
                Err(e) => {
                    warn!("Error while parsing: {:?}", e);
                    thread::sleep(Duration::from_millis(500))
                }
            }

        }

        self.print_stats();
        Ok(())
    }
    
    pub fn print_stats(&self) {
        let ticks_per_ns = self.interpolation_points.get_avg_ticks_per_ns();
        info!("Printing stats...");
        
        let mut total_events = 0;
        for (ord_id, thread) in &self.event_parsers {
            info!("\tThread: {:?}#{:?}", thread.thread_name, ord_id);
            let stats = thread.stats;
            
            let events_per_sec = stats.total_events as f64 / ((stats.max_timestamp - stats.min_timestamp) as f64 / ticks_per_ns) * 1_000_000_000.0;
            let events_per_sec_covered = stats.total_events as f64 / (stats.covered_dur as f64 / ticks_per_ns) * 1_000_000_000.0;
            info!("Total events: {}", stats.total_events);
            info!("Events per second (global): {} eps", events_per_sec);
            info!("Events per second (covered): {} eps", events_per_sec_covered);
            info!("Average event duration: {} ns", stats.covered_dur as f64 / ticks_per_ns / stats.total_events as f64);
            
            info!("\n");
            
            total_events += stats.total_events;
        }
        
        info!("Average bytes per event: {} bytes", self.packet_decoder.counters().trace_buf as f64 / total_events as f64);
        info!("Average transport bytes per event: {} bytes", self.packet_decoder.counters().total_bytes() as f64 / total_events as f64);

    }

    /// Continuously pull events until EOF.
    /// Decode incoming events and save them to `trace.json` in Perfetto format
    pub fn parse_and_convert_to_perfetto(&mut self) -> ParseResult<BytesMut> {
        let mut trace_res_file = PerfettoTraceFile::new();
        self.parse_to_end(|ev, thread_info| {
            let thread_id = thread_info.thread_id.unwrap_or(999);
            let thread_name = thread_info.thread_name.clone().unwrap_or("Unknown thread".to_string());
            trace_res_file.set_thread_name(thread_id, &thread_name);
            trace_res_file.set_thread_name(999666 + thread_info.thread_ord_id, "[not thread] local packets");
            
            match ev {
                ParsedEvent::Instant {
                    name,
                    tm
                } => {
                    trace_res_file.add_point_event(&name, thread_id, *tm);
                }
                ParsedEvent::Range {
                    name,
                    start,
                    end
                } => {
                    trace_res_file.add_range_event(&name, thread_id,
                                                   *start, *end);
                }
                ParsedEvent::NamedRange {
                    name,
                    end_name,
                    start,
                    end
                } => {

                    trace_res_file.add_range_event(&format!("{} -> {}", name, end_name), thread_id, 
                                                   *start, *end);
                }
            }
        })?;

        if cfg!(feature="local-packet-bounds") {
            for (global_i, local_i, thread_ord_id, start,end) in mem::take(&mut self.local_packet_ranges).into_iter() {
                trace_res_file.add_range_event(&format!("Local packet #{global_i}.{local_i}"), 999666 + thread_ord_id, start, end);
            }
        }

        let encoder_info = self.machine_info.take().unwrap_or_else(|| {
            warn!("Encoder info is not present in decoded data! Using default values");
            SparklesMachineInfo::default()
        });
        trace_res_file.set_process_info(encoder_info.process_name, encoder_info.pid);

        let bytes = trace_res_file.get_bytes();
        Ok(bytes)
    }
    fn thread_parser_state(&mut self, thread_id: u64) -> &mut ThreadParserState {
        self.event_parsers.entry(thread_id).or_default()
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