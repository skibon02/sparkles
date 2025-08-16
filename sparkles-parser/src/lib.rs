#[cfg(feature="perfetto")]
mod perfetto_format;
pub mod tracing_decoder;
pub mod parsed;
pub mod packet_decoder;
pub mod discovery_wrapper;

use std::collections::BTreeMap;
use std::ops::Deref;
use std::rc::Rc;
use std::thread;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use indexmap::IndexMap;
use log::{debug, error, info, warn};
use sparkles_core::consts::PROTOCOL_VERSION;
use sparkles_core::local_storage::id_mapping::EventType;
use sparkles_core::protocol::headers::SparklesMachineInfo;
use crate::packet_decoder::{Packet, PacketDecoder, PacketReadError, ProtocolCounters};
use crate::parsed::{ParsedEvent, ThreadInfoState};
use crate::tracing_decoder::StreamFrameDecoder;

// pub exports
pub use discovery_wrapper::DiscoveryWrapper;

pub static PARSER_BUF_SIZE: usize = 1_000_000;
static SHUTDOWN_SIGNAL: AtomicBool = AtomicBool::new(false);

pub fn request_shutdown() {
    SHUTDOWN_SIGNAL.store(true, std::sync::atomic::Ordering::SeqCst);
    info!("Shutdown requested...")
}
pub fn is_shutting_down() -> bool {
    SHUTDOWN_SIGNAL.load(std::sync::atomic::Ordering::SeqCst)
}

pub struct SparklesParser {
    machine_info: Option<SparklesMachineInfo>,

    event_parsers: BTreeMap<u64, ThreadParserState>,
    local_packet_ranges: Vec<(usize, usize, u64, u64, u64)>,
    global_i: usize,
    interpolation_points: InterpolationPoints,
    counters: ProtocolCounters
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

#[derive(Debug)]
struct ForeignRangeEnd {
    event_id: Option<TracingEventId>,
    timestamp: u64,
    ord_id: u8,
    foreign_thread_ord_id: u64,
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
    // Storage for foreign range ends to be processed later
    foreign_range_ends: Vec<ForeignRangeEnd>,
    zero_diff_cnt: u64,
    
    id_store: IndexMap<TracingEventId, (Rc<str>, EventType)>,
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
    pub fn new() -> Self {
        #[cfg(feature="self-tracing")]
        sparkles::init(sparkles::config::SparklesConfig::default()
            .with_global_capacity(100_000)
            .with_file_sender(sparkles::sender::file_sender::FileSenderConfig::Directory(std::path::PathBuf::from("parser-trace")))
        ).forget();
        Self {
            machine_info: None,
            event_parsers: BTreeMap::new(),

            local_packet_ranges: Vec::new(),
            global_i: 0,
            interpolation_points: InterpolationPoints::new(),
            counters: ProtocolCounters::default()
        }
    }

    pub fn parse_to_end(&mut self,
                        mut packet_decoder: PacketDecoder,
                        mut f: impl FnMut(&[ParsedEvent], &ThreadInfoState, &IndexMap<TracingEventId, (Rc<str>, EventType)>),
                        mut on_event_names_changed: impl FnMut(&ThreadInfoState, &IndexMap<TracingEventId, (Rc<str>, EventType)>)
    ) -> ParseResult<()> {
        let (packets_tx, packets_rx) = mpsc::sync_channel(100);
        let (counters_tx, counters_rx) = mpsc::sync_channel(1);

        let jh = thread::Builder::new().name(String::from("Receiving thread")).spawn(move || {
            let mut last_packet_received = Instant::now();
            loop {
                let packet = packet_decoder.read_packet();
                #[cfg(feature="self-tracing")]
                let g = sparkles_macro::range_event_start!("Parse packet");

                if packet.is_ok() {
                    last_packet_received = Instant::now();
                }
                match packet {
                    Ok(Some(packet)) => {
                        if matches!(packet, Packet::GracefulShutdown) {
                            info!("GracefulShutdown received!");
                            break;
                        }
                        packets_tx.send(packet).unwrap();
                        #[cfg(feature="self-tracing")]
                        sparkles_macro::range_event_end!(g, "Got packet!");
                    }
                    Ok(None) => {

                    }
                    Err(e) => {
                        #[cfg(feature="self-tracing")]
                        sparkles_macro::range_event_end!(g, "Error while reading");
                        warn!("Error while reading packet: {e:?}");
                        
                        thread::sleep(Duration::from_millis(500));

                        if last_packet_received.elapsed() > Duration::from_secs(5) {
                            warn!("No packets received for 5 seconds. Exiting...");
                            break;
                        }
                    }
                }

                if SHUTDOWN_SIGNAL.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
            }
            counters_tx.send(packet_decoder.counters()).unwrap();
        }).unwrap();

        while let Ok(packet) = packets_rx.recv() {
            self.parse_single_packet(packet, &mut f, &mut on_event_names_changed);
        }
        self.counters = counters_rx.recv().unwrap();

        jh.join().unwrap();

        #[cfg(feature="self-tracing")]
        sparkles::finalize();

        self.print_stats();
        Ok(())
    }

    pub fn parse_single_packet(&mut self,
                               packet: Packet,
                               on_new_events: &mut impl FnMut(&[ParsedEvent], &ThreadInfoState, &IndexMap<TracingEventId, (Rc<str>, EventType)>),
                               on_event_names_changed: &mut impl FnMut(&ThreadInfoState, &IndexMap<TracingEventId, (Rc<str>, EventType)>)
    ) {
        match packet {
            Packet::MachineInfo(info) => {
                if info.ver.0 != PROTOCOL_VERSION.0 {
                    error!("Protocol major version mismatch! Parser: {}, Sender: {}", PROTOCOL_VERSION.0, info.ver.0);
                }
                else if info.ver.1 > PROTOCOL_VERSION.1 {
                    warn!("Sender protocol version is higher than parser! Parser: {}.{}, Sender: {}.{}",
                        info.ver.0, PROTOCOL_VERSION.1, info.ver.0, info.ver.1)
                }

                self.machine_info = Some(info);
            }
            Packet::TimestampFreq(ticks_per_sec, cur_tm) => {
                let ticks_per_ns = ticks_per_sec as f64 / 1_000_000_000.0;
                info!("Got timestamp frequency: {ticks_per_ns:?} t/ns");

                self.interpolation_points.add_interpolation_point(ticks_per_ns, cur_tm);
            }
            Packet::DataBytes(packets) => {
                if self.interpolation_points.is_empty() {
                    error!("Timestamp frequency is not set! Dropping packet.");
                }
                
                let global_i = self.global_i;
                self.global_i += 1;
                for (local_i, (header, data)) in packets.into_iter().enumerate() {
                    #[cfg(feature="self-tracing")]
                    let g = sparkles_macro::range_event_start!("Parse header");
                    let thread_id = header.thread_ord_id;
                    let parser_state = self.event_parsers.entry(thread_id).or_default();
                    self.local_packet_ranges.push((global_i, local_i, thread_id, self.interpolation_points.project_tm(header.start_timestamp), self.interpolation_points.project_tm(header.end_timestamp)));

                    //update thread name
                    parser_state.thread_id = Some(header.thread_info.thread_id);
                    if let Some(thread_name) = header.thread_info.new_thread_name.clone() {
                        parser_state.thread_name = Some(thread_name);
                    }
                    parser_state.last_thread_ord_id = thread_id;

                    // Merge id store
                    let mut something_changed = false;
                    for (id, (name, r#type)) in header.id_store.tags.iter().enumerate() {
                        let id = id as u8;
                        if let Some((old_name, old_type)) = parser_state.id_store.get(&id) {
                            if old_name.as_ref() != name.deref() || old_type != r#type {
                                something_changed = true;
                                error!("ID store mismatch for thread {:?}#{:?}! ID: {}, Old: {:?}, New: {:?}", parser_state.thread_name, parser_state.thread_id,
                                                id, (old_name, old_type), (name, r#type));
                            }
                        }
                        else {
                            something_changed = true;
                        }
                        parser_state.id_store.insert(id, (Rc::from(name.deref()), *r#type));
                    }
                    #[cfg(feature="self-tracing")]
                    drop(g);

                    if something_changed {
                        on_event_names_changed(&parser_state.thread_info_state(), &parser_state.id_store);
                    }

                    if self.interpolation_points.is_empty() {
                        continue;
                    }

                    #[cfg(feature="self-tracing")]
                    let g = sparkles_macro::range_event_start!("Decode raw events");
                    let new_events = parser_state.state_machine.decode_many(&data);
                    let new_events_len = new_events.len();
                    debug!("Received {new_events_len} events");
                    #[cfg(feature="self-tracing")]
                    drop(g);

                    #[cfg(feature="self-tracing")]
                    let g = sparkles_macro::range_event_start!("Parse new events");
                    let mut cur_tm = header.start_timestamp;
                    let mut first = true;

                    let mut res = Vec::with_capacity(new_events_len / 2);
                    for evt in new_events {
                        let mut dif_tm_zero = false;
                        if first {
                            first = false;
                        }
                        else {
                            let dif_tm = match evt {
                                TracingEvent::Instant(_, dif_tm) => dif_tm,
                                TracingEvent::RangePart(_, dif_tm, _) => dif_tm,
                                TracingEvent::UnnamedRangeEnd(dif_tm, _) => dif_tm,
                                TracingEvent::ForeignRangeEnd(_, dif_tm, _, _) => dif_tm,
                            };
                            if dif_tm == 0 {
                                dif_tm_zero = true;
                            }
                            cur_tm += dif_tm;
                        }
                        if !dif_tm_zero {
                            parser_state.zero_diff_cnt = 0;
                        }
                        else {
                            parser_state.zero_diff_cnt += 1;
                        }
                        if cur_tm > header.end_timestamp {
                            warn!("Parsing issue: Timestamp is outside local packet! diff: {}",  cur_tm - header.end_timestamp);
                        }

                        // Create ParsedEvent
                        let timestamp = self.interpolation_points.project_tm(cur_tm) + parser_state.zero_diff_cnt * 10;
                        match evt {
                            TracingEvent::Instant(id, _) => {
                                let ev_name = if let Some((ev_name, ev_type)) = parser_state.id_store.get(&id) {
                                    if ev_type != &EventType::Instant {
                                        error!("Assertion failed: Instant event type is not Instant!");
                                    }
                                    ev_name.clone()
                                }
                                else {
                                    error!("Did not find event name for id: {id}");
                                    Rc::from(format!("Unknown Instant {id}"))
                                };
                                let parsed = ParsedEvent::Instant {
                                    name_id: id,
                                    tm: timestamp
                                };
                                res.push(parsed);
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
                                                let parsed = ParsedEvent::Range {
                                                    name_id: *start_id,
                                                    end_name_id: Some(id),
                                                    start: start_tm,
                                                    end: timestamp,
                                                    start_thread_ord_id: None,
                                                };
                                                res.push(parsed);
                                            }
                                            else {
                                                warn!("Did not find start event for RangePart id: {id}");
                                            }
                                        }
                                        else {
                                            warn!("Did not find start event for RangePart id: {id}");
                                        }
                                    }
                                    else {
                                        // Range start
                                        parser_state.cur_started_ranges.insert(ord_id, (id, timestamp));
                                    }
                                }
                                else {
                                    error!("Did not find event name for id: {id}");
                                    let ev_name: Rc<str> = Rc::from(format!("Unknown RangePart {id}"));

                                    let parsed = ParsedEvent::Instant {
                                        name_id: id,
                                        tm: timestamp
                                    };
                                    res.push(parsed);
                                };

                            }
                            TracingEvent::UnnamedRangeEnd(_, ord_id ) => {
                                if let Some(start_info) = parser_state.cur_started_ranges.remove(&ord_id) {
                                    if let Some((start_name, ev_type)) = parser_state.id_store.get(&start_info.0) {
                                        if *ev_type != EventType::RangeStart {
                                            error!("Assertion failed: UnnamedRangeEnd event has non-RangeStart type!");
                                        }
                                        let parsed = ParsedEvent::Range {
                                            name_id: start_info.0,
                                            end_name_id: None,
                                            start: start_info.1,
                                            end: timestamp,
                                            start_thread_ord_id: None,
                                        };
                                        res.push(parsed);
                                    }
                                    else {
                                        warn!("Did not find start event for UnnamedRangeEnd id: {ord_id}. Skipping...");
                                    }
                                }
                                else {
                                    warn!("Did not find start event for UnnamedRangeEnd id: {ord_id}. Skipping...");
                                }
                            }
                            TracingEvent::ForeignRangeEnd(id, _, ord_id, foreign_thread_id) => {
                                parser_state.foreign_range_ends.push(ForeignRangeEnd {
                                    event_id: id,
                                    timestamp,
                                    ord_id,
                                    foreign_thread_ord_id: foreign_thread_id,
                                });
                            }
                        }
                    }

                    // Process foreign range ends after regular events
                    let mut foreign_events = Vec::new();
                    let foreign_ends_to_process: Vec<_> = parser_state.foreign_range_ends.drain(..).collect();
                    let mut remaining_foreign_ends = Vec::new();
                    
                    for foreign_end in foreign_ends_to_process {
                        if let Some(foreign_parser_state) = self.event_parsers.get_mut(&foreign_end.foreign_thread_ord_id) {
                            if let Some((start_event_id, start_timestamp)) = foreign_parser_state.cur_started_ranges.remove(&foreign_end.ord_id) {
                                let parsed_event = ParsedEvent::Range {
                                    name_id: start_event_id,
                                    end_name_id: foreign_end.event_id,
                                    start: start_timestamp,
                                    end: foreign_end.timestamp,
                                    start_thread_ord_id: Some(foreign_end.foreign_thread_ord_id),
                                };
                                foreign_events.push(parsed_event);
                            } else {
                                remaining_foreign_ends.push(foreign_end);
                            }
                        } else {
                            remaining_foreign_ends.push(foreign_end);
                        }
                    }
                    
                    let parser_state = self.event_parsers.get_mut(&thread_id).unwrap();
                    parser_state.foreign_range_ends = remaining_foreign_ends;
                    
                    if !foreign_events.is_empty() {
                        on_new_events(&foreign_events, &parser_state.thread_info_state(), &parser_state.id_store);
                    }

                    on_new_events(&res, &parser_state.thread_info_state(), &parser_state.id_store);

                    parser_state.stats.new_events(new_events_len, header.start_timestamp, header.end_timestamp);

                    parser_state.state_machine.ensure_buf_end();
                }
            }

            Packet::FailedPages(failed_pages) => {
                for header in failed_pages {
                    info!("Got failed pages header: {header:?}");

                    let start = header.start_timestamp;
                    let dur = header.end_timestamp - header.start_timestamp;
                    let thread_ord_id = header.thread_ord_id;
                    self.thread_parser_state(thread_ord_id).missed_events.push((start, dur));
                }
            }

            Packet::GracefulShutdown => {}
            Packet::ConnectionAccepted => {}
            Packet::Hello => {}
        }
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
            info!("Events per second (global): {events_per_sec} eps");
            info!("Events per second (covered): {events_per_sec_covered} eps");
            info!("Average event duration: {} ns", stats.covered_dur as f64 / ticks_per_ns / stats.total_events as f64);
            
            info!("\n");
            
            total_events += stats.total_events;
        }
        
        info!("Average bytes per event: {} bytes", self.counters.trace_buf as f64 / total_events as f64);
        info!("Average transport bytes per event: {} bytes", self.counters.total_bytes() as f64 / total_events as f64);

    }

    /// Continuously pull events until EOF.
    /// Decode incoming events and save them to `trace.json` in Perfetto format
    #[cfg(feature="perfetto")]
    pub fn parse_and_convert_to_perfetto(&mut self, packet_decoder: PacketDecoder) -> ParseResult<bytes::BytesMut> {
        use crate::perfetto_format::PerfettoTraceFile;
        use std::collections::HashMap;

        let mut trace_res_file = PerfettoTraceFile::new();

        let mut per_thread_info = HashMap::new();
        let mut cross_thread_ranges = Vec::new();
        self.parse_to_end(packet_decoder, |evs, thread_info, event_names| {
            let thread_id = thread_info.thread_id.unwrap_or(999);
            trace_res_file.set_thread_name(thread_id, thread_info.thread_name.as_deref());
            #[cfg(feature="local-packet-bounds")]
            trace_res_file.set_thread_name(999666 + thread_info.thread_ord_id, Some("[not thread] local packets"));

            per_thread_info.insert(thread_info.thread_ord_id, (event_names.clone(), thread_info.thread_name.clone().unwrap_or_else(|| "Unknown".to_string())));

            for ev in evs {
                match ev {
                    ParsedEvent::Instant {
                        name_id,
                        tm
                    } => {
                        let name = &event_names.get(name_id).unwrap().0;
                        trace_res_file.add_point_event(name, thread_id, *tm);
                    }
                    ParsedEvent::Range {
                        name_id,
                        end_name_id,
                        start,
                        end,
                        start_thread_ord_id
                    } => {
                        if let Some(start_thread_ord_id) = start_thread_ord_id {
                            let end_name = end_name_id.and_then(|id| event_names.get(&id).map(|(name, _)| name.to_string()));
                            cross_thread_ranges.push((*name_id, end_name, *start, *end, *start_thread_ord_id, thread_id));
                        } else {
                            let name = &event_names.get(name_id).unwrap().0;
                            let display_name = if let Some(end_name_id) = end_name_id {
                                let end_name = &event_names.get(end_name_id).unwrap().0;
                                format!("{name} -> {end_name}")
                            } else {
                                name.to_string()
                            };
                            trace_res_file.add_range_event(&display_name, thread_id, *start, *end);
                        }
                    }
                }
            }
        }, |_, _| {})?;

        // Process cross-thread range events after main parsing
        for (start_name_id, end_name, start_tm, end_tm, start_thread_ord_id, end_thread_id) in cross_thread_ranges {
            let (start_name, thread_name) = if let Some((start_event_names, start_thread_name)) = per_thread_info.get(&start_thread_ord_id) {
                if let Some((start_name, _)) = start_event_names.get(&start_name_id) {
                    (start_name.as_ref(), start_thread_name.as_str())
                } else {
                    warn!("Could not find start event name for cross-thread range: start_name_id={}", start_name_id);
                    ("Unknown", start_thread_name.as_str())
                }
            } else {
                warn!("Could not find start thread info for cross-thread range: start_thread_ord_id={}", start_thread_ord_id);
                ("Unknown", "cross-thread")
            };

            let display_name = if let Some(ref end_name) = end_name {
                format!("{start_name} [{thread_name}] -> {end_name}")
            } else {
                format!("{start_name} [{thread_name}]")
            };
            trace_res_file.add_range_event(&display_name, end_thread_id, start_tm, end_tm);
        }

        if cfg!(feature="local-packet-bounds") {
            for (global_i, local_i, thread_ord_id, start,end) in std::mem::take(&mut self.local_packet_ranges).into_iter() {
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
    UnnamedRangeEnd(u64, u8),
    ForeignRangeEnd(Option<TracingEventId>, u64, u8, u64),
}
const VERSION: &str = env!("CARGO_PKG_VERSION");
pub fn version() {
    println!("Sparkles-parser v{VERSION}");
    println!("  Using sparkles protocol version {}.{}", PROTOCOL_VERSION.0, PROTOCOL_VERSION.1);
}
