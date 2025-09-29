#[cfg(feature="perfetto")]
mod perfetto_format;
pub mod parsed;
pub mod packet_decoder;
pub mod discovery_wrapper;
pub mod parser;
pub mod time_sync;

use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};
use std::thread;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use indexmap::IndexMap;
use log::{error, info, warn};
use sparkles_core::consts::PROTOCOL_VERSION;
use sparkles_core::protocol::headers::SparklesMachineInfo;
use crate::packet_decoder::{Packet, PacketDecoder, PacketReadError, ProtocolCounters};
use crate::parsed::{ExternalChannelInfo, ParsedEvent, ParsedExternalEvent, ThreadInfo};
use crate::parser::thread_parser::{EventNamesStore, ThreadParserEvent, ThreadParserState};

// pub exports
pub use discovery_wrapper::DiscoveryWrapper;
use crate::time_sync::{TimeSyncPoints, MonotonicTimeSyncPoints};
use crate::parser::external_parser::{ExternalEventNamesStore, ExternalParserEvent, ExternalParserState};

pub static PARSER_BUF_SIZE: usize = 1_000_000;
static SHUTDOWN_SIGNAL: AtomicBool = AtomicBool::new(false);

pub fn request_shutdown() {
    SHUTDOWN_SIGNAL.store(true, std::sync::atomic::Ordering::SeqCst);
    info!("Shutdown requested...")
}
pub fn is_shutting_down() -> bool {
    SHUTDOWN_SIGNAL.load(std::sync::atomic::Ordering::SeqCst)
}

pub enum SparklesParserEvent<'a> {
    ThreadParserEvent(ThreadParserEvent, &'a ThreadInfo),
    ExternalParserEvent(ExternalParserEvent, &'a ExternalChannelInfo),
}

pub struct SparklesParser {
    machine_info: Option<SparklesMachineInfo>,

    event_parsers: EventParsers,
    external_event_parsers: ExternalEventParsers,
    local_packet_ranges: Vec<(usize, usize, u64, u64, u64)>,
    global_i: usize,
    // Synchronization points between monotonic clock and CPU clock
    time_sync_points: MonotonicTimeSyncPoints,
    counters: ProtocolCounters
}

#[derive(Default)]
pub struct EventParsers(BTreeMap<u64, ThreadParserState>);
impl EventParsers {
    pub fn entry(&mut self, thread_id: u64) -> &mut ThreadParserState {
        self.0.entry(thread_id).or_insert(ThreadParserState::new(thread_id))
    }
}
impl Deref for EventParsers {
    type Target = BTreeMap<u64, ThreadParserState>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for EventParsers {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
#[derive(Default)]
pub struct ExternalEventParsers(BTreeMap<u32, ExternalParserState>);
impl ExternalEventParsers {
    pub fn entry(&mut self, ext_ord_id: u32) -> &mut ExternalParserState {
        self.0.entry(ext_ord_id).or_insert(ExternalParserState::new(ext_ord_id))
    }
    pub fn keys(&self) -> impl Iterator<Item=&u32> {
        self.0.keys()
    }
}
impl Deref for ExternalEventParsers {
    type Target = BTreeMap<u32, ExternalParserState>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for ExternalEventParsers {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug)]
pub struct ForeignRangeEnd {
    event_id: Option<EventNameId>,
    timestamp: u64,
    ord_id: u8,
    foreign_thread_ord_id: u64,
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
            event_parsers: EventParsers::default(),
            external_event_parsers: ExternalEventParsers::default(),

            local_packet_ranges: Vec::new(),
            global_i: 0,
            time_sync_points: MonotonicTimeSyncPoints::new(),
            counters: ProtocolCounters::default(),
        }
    }

    pub fn parse_to_end(&mut self,
                        mut packet_decoder: PacketDecoder,
                        mut on_new_event: impl FnMut(SparklesParserEvent),
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
            self.parse_single_packet(packet, &mut on_new_event);
        }
        // parse remaining unhandled packets
        for thread_state in self.event_parsers.values_mut() {
            if let Some(events) = thread_state.parse_unhandled_events(&self.time_sync_points, true) && !events.is_empty() {
                on_new_event(SparklesParserEvent::ThreadParserEvent(ThreadParserEvent::NewEvents(events), &thread_state.thread_info()) );
            }
            let unhandled_events_count = thread_state.unhandled_events_count();
            if unhandled_events_count > 0 {
                warn!("Thread {} still has {unhandled_events_count} unhandled events after final parsing!", thread_state.ord_id());
            }
        }

        for ext_state in self.external_event_parsers.values_mut() {
            if let Some(events) = ext_state.parse_unhandled_events(true) && !events.is_empty() {
                on_new_event(SparklesParserEvent::ExternalParserEvent(ExternalParserEvent::NewEvents(events), &ext_state.channel_info()) );
            }
            let unhandled_events_count = ext_state.unhandled_events_count();
            if unhandled_events_count > 0 {
                warn!("External channel {} still has {unhandled_events_count} unhandled events after final parsing!", ext_state.ord_id());
            }
        }

        // Process foreign range ends
        let thread_ord_id_keys = self.event_parsers.keys().cloned().collect::<Vec<_>>();
        for thread_ord_id in thread_ord_id_keys {
            if let Some(foreign_events) = Self::process_foreign_ends(&mut self.event_parsers, thread_ord_id) {
                on_new_event(SparklesParserEvent::ThreadParserEvent(ThreadParserEvent::NewEvents(foreign_events), &self.event_parsers.entry(thread_ord_id).thread_info()));
            }
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
                               on_new_event: &mut impl FnMut(SparklesParserEvent)
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
            Packet::SyncPoint(monotonic_tm, cur_tm) => {
                self.time_sync_points.add_time_sync_point(monotonic_tm, cur_tm);
            }
            Packet::DataBytes(packets) => {
                let global_i = self.global_i;
                self.global_i += 1;
                for (local_i, (header, data)) in packets.into_iter().enumerate() {
                    #[cfg(feature="self-tracing")]
                    let g = sparkles_macro::range_event_start!("Parse header");
                    let thread_id = header.thread_ord_id;
                    self.local_packet_ranges.push((global_i, local_i, thread_id, header.start_timestamp, header.end_timestamp));
                    let parser_state = self.event_parsers.entry(thread_id);
                    let events = parser_state.got_events(header, data, &self.time_sync_points);
                    for event in events {
                        on_new_event(SparklesParserEvent::ThreadParserEvent(event, &parser_state.thread_info()) );
                    }

                    // handle foreign events
                    if let Some(foreign_events) = Self::process_foreign_ends(&mut self.event_parsers, thread_id) {
                        let parser_state = self.event_parsers.entry(thread_id);
                        on_new_event(SparklesParserEvent::ThreadParserEvent(ThreadParserEvent::NewEvents(foreign_events), &parser_state.thread_info()));
                    }
                }
            }

            Packet::FailedPages(failed_pages) => {
                for header in failed_pages {
                    info!("Got failed pages header: {header:?}");

                    let start = header.start_timestamp;
                    let dur = header.end_timestamp - header.start_timestamp;
                    let thread_ord_id = header.thread_ord_id;
                    self.thread_parser_state(thread_ord_id).got_missed_events(start, dur);
                }
            }
            Packet::ExternalEventNames(names) => {
                let id = names.ext_ord_id;
                let parser_state = self.external_event_parsers.entry(id);
                let channel_info = parser_state.channel_info();

                for event in parser_state.got_event_names(names) {
                    on_new_event(SparklesParserEvent::ExternalParserEvent(event, &channel_info));
                }
            }

            Packet::ExternalSyncPoint(ext_ord_id, local_tm, external_tm) => {
                let parser_state = self.external_event_parsers.entry(ext_ord_id);
                parser_state.add_time_sync_point(local_tm, external_tm);
            }
            Packet::ExternalEvents(header, events) => {
                let id = header.ext_ord_id;
                let parser_state = self.external_event_parsers.entry(id);
                let channel_info = parser_state.channel_info();

                for event in parser_state.got_events(header, &events) {
                    on_new_event(SparklesParserEvent::ExternalParserEvent(event, &channel_info));
                }
            }

            Packet::GracefulShutdown => {}
            Packet::ConnectionAccepted => {}
            Packet::Hello => {}
        }
    }

    fn process_foreign_ends(event_parsers: &mut EventParsers, thread_id: u64) -> Option<Vec<ParsedEvent>>{
        // Process foreign range ends after regular events
        let parser_state = event_parsers.entry(thread_id);
        let foreign_ends = parser_state.take_foreign_range_ends();

        let mut foreign_events = vec![];
        let mut remaining_foreign_ends = Vec::new();

        for foreign_end in foreign_ends {
            if let Some(foreign_parser_state) = event_parsers.get_mut(&foreign_end.foreign_thread_ord_id) {
                if let Some((start_event_id, start_timestamp)) = foreign_parser_state.remove_foreign_range(foreign_end.ord_id) {
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

        // Store remaining foreign ends back to the parser state
        let parser_state = event_parsers.get_mut(&thread_id).unwrap();
        parser_state.store_foreign_ends(remaining_foreign_ends);

        if !foreign_events.is_empty() {
            Some(foreign_events)
        } else {
            None
        }
    }


    pub fn print_stats(&self) {
        let ticks_per_ns = self.time_sync_points.get_avg_ticks_per_ns().unwrap_or(0.0);
        info!("Printing stats...");
        
        let mut total_events = 0;
        for (ord_id, thread) in self.event_parsers.deref() {
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

        let mut per_thread_info: HashMap<u64, (EventNamesStore, Arc<str>)> = HashMap::new();
        let mut per_channel_info: HashMap<u32, (ExternalEventNamesStore, Arc<str>)> = HashMap::new();
        let mut cross_thread_ranges = Vec::new();
        self.parse_to_end(packet_decoder, |event| {
            match event {
                SparklesParserEvent::ThreadParserEvent(evt, thread_info) => {
                    match evt {
                        ThreadParserEvent::NewEvents(events) => {
                            let thread_id = thread_info.thread_id.unwrap_or(999);
                            trace_res_file.set_thread_name(thread_id, thread_info.thread_name.as_deref());
                            #[cfg(feature="local-packet-bounds")]
                            trace_res_file.set_thread_name(999666 + thread_info.thread_ord_id, Some("[not thread] local packets"));

                            let event_names = if let Some((names, _)) = per_thread_info.get(&thread_info.thread_ord_id) {
                                names
                            } else {
                                warn!("Event names for thread_ord_id={} not found! Using empty names.", thread_info.thread_ord_id);
                                &IndexMap::new()
                            };

                            for ev in events {
                                match ev {
                                    ParsedEvent::Instant {
                                        name_id,
                                        tm
                                    } => {
                                        let name = &event_names.get(&name_id).unwrap().0;
                                        trace_res_file.add_point_event(name, thread_id, tm);
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
                                            cross_thread_ranges.push((name_id, end_name, start, end, start_thread_ord_id, thread_id));
                                        } else {
                                            let name = &event_names.get(&name_id).unwrap().0;
                                            let display_name = if let Some(end_name_id) = end_name_id {
                                                let end_name = &event_names.get(&end_name_id).unwrap().0;
                                                format!("{name} -> {end_name}")
                                            } else {
                                                name.to_string()
                                            };
                                            trace_res_file.add_range_event(&display_name, thread_id, start, end);
                                        }
                                    }
                                }
                            }
                        }
                        ThreadParserEvent::EventNamesChanged(event_names) => {
                            per_thread_info.insert(thread_info.thread_ord_id, (event_names.clone(), thread_info.thread_name.clone().unwrap_or_else(|| "Unknown".to_string()).into()));
                        }
                    }
                }
                SparklesParserEvent::ExternalParserEvent(evt, channel_info) => {
                    match evt {
                        ExternalParserEvent::NewEvents(events) => {
                            let thread_id = channel_info.ext_ord_id as u64 + 11_000_000;
                            trace_res_file.set_thread_name(thread_id, Some(channel_info.channel_name.as_deref().unwrap_or("External channel")));

                            let event_names = if let Some((names, _)) = per_channel_info.get(&(channel_info.ext_ord_id)) {
                                names
                            } else {
                                warn!("Event names for external channel_ord_id={} not found! Using empty names.", channel_info.ext_ord_id);
                                &IndexMap::new()
                            };

                            for ev in events {
                                match ev {
                                    ParsedExternalEvent::Instant {
                                        name_id,
                                        tm
                                    } => {
                                        let name = &event_names.get(&name_id).unwrap();
                                        trace_res_file.add_point_event(name, thread_id, tm);
                                    }
                                    ParsedExternalEvent::Range {
                                        name_id,
                                        end_name_id,
                                        start,
                                        end,
                                    } => {
                                        let name = &event_names.get(&name_id).unwrap();
                                        let display_name = if end_name_id.is_some_and(|id| id != name_id) {
                                            let end_name = &event_names.get(&end_name_id.unwrap()).unwrap();
                                            format!("{name} -> {end_name}")
                                        } else {
                                            name.to_string()
                                        };
                                        trace_res_file.add_range_event(&display_name, thread_id, start, end);
                                    }
                                }
                            }
                        }
                        ExternalParserEvent::NewEventNames(event_names) => {
                            per_channel_info.insert(channel_info.ext_ord_id, (event_names.clone(), channel_info.channel_name.clone().unwrap_or_else(|| "External channel".to_string().into())));
                        }
                    }
                }
            }
        })?;

        // Process cross-thread range events after main parsing
        for (start_name_id, end_name, start_tm, end_tm, start_thread_ord_id, end_thread_id) in cross_thread_ranges {
            let (start_name, thread_name) = if let Some((start_event_names, start_thread_name)) = per_thread_info.get(&start_thread_ord_id) {
                if let Some((start_name, _)) = start_event_names.get(&start_name_id) {
                    (start_name.as_ref(), start_thread_name.deref())
                } else {
                    warn!("Could not find start event name for cross-thread range: start_name_id={}", start_name_id);
                    ("Unknown", start_thread_name.deref())
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
            for (global_i, local_i, thread_ord_id, start_cpu,end_cpu) in std::mem::take(&mut self.local_packet_ranges).into_iter() {
                let start_tm = self.time_sync_points.project_tm(start_cpu);
                let end_tm = self.time_sync_points.project_tm(end_cpu);
                if let (Some(start), Some(end)) = (start_tm, end_tm) {
                    trace_res_file.add_range_event(&format!("Local packet #{global_i}.{local_i}"), 999666 + thread_ord_id, start, end);
                }
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
        self.event_parsers.entry(thread_id)
    }
}

pub type EventNameId = u8;
pub type ExternalEventNameId = u16;

const VERSION: &str = env!("CARGO_PKG_VERSION");
pub fn version() {
    println!("Sparkles-parser v{VERSION}");
    println!("  Using sparkles protocol version {}.{}", PROTOCOL_VERSION.0, PROTOCOL_VERSION.1);
}
