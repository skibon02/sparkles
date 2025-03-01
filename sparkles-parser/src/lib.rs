mod perfetto_format;
mod consts;
pub mod tracing_decoder;
pub mod parsed;
pub mod packet_decoder;

use std::collections::BTreeMap;
use std::io::Read;
use std::net::SocketAddr;
use std::thread;
use std::time::Duration;
use bytes::BytesMut;
use log::{debug, info, warn};
use sparkles_core::protocol::headers::{LocalPacketHeader, SparklesMachineInfo};
use crate::packet_decoder::{Packet, PacketDecoder, PacketReadError};
use crate::parsed::{ParsedEvent, ParsedEventGroup, ThreadInfoState};
use crate::tracing_decoder::StreamFrameDecoder;
use crate::perfetto_format::PerfettoTraceFile;

pub static PARSER_BUF_SIZE: usize = 1_000_000;

pub struct SparklesParser {
    packet_decoder: PacketDecoder,

    machine_info: Option<SparklesMachineInfo>,
    ticks_per_ns: Option<f64>,

    event_parsers: BTreeMap<u64, ThreadParserState>,
}

#[derive(Default)]
pub struct ThreadParserState {
    thread_name: Option<String>,
    thread_id: Option<u64>,
    event_buf: Vec<(LocalPacketHeader, Vec<TracingEvent>)>,

    // start timestamp and duration for missed events packet
    missed_events: Vec<(u64, u64)>,

    // ---- TMP DATA ----
    state_machine: StreamFrameDecoder,
    // Helper for ranges handling
    cur_started_ranges: BTreeMap<u8, (TracingEventId, u64)>,
    // Current timestamp, accumulated from events
    cur_tm: u64,
    zero_diff_cnt: u64,
}

pub type ParseResult<T> = Result<T, PacketReadError>;

impl SparklesParser {
    /// Initialize parser from byte stream
    pub fn from_decoder(decoder: PacketDecoder) -> Self {
        Self {
            packet_decoder: decoder,

            machine_info: None,
            event_parsers: BTreeMap::new(),
            ticks_per_ns: None,
        }
    }

    pub fn from_stream(stream: impl Read + 'static) -> Self {
        let decoder = PacketDecoder::from_stream(stream);
        Self::from_decoder(decoder)
    }

    pub fn from_udp_addr(addr: SocketAddr) -> Self {
        let decoder = PacketDecoder::from_socket(addr);
        Self::from_decoder(decoder)
    }

    pub fn is_eof(&self) -> bool {
        self.packet_decoder.is_eof()
    }

    pub fn parse_to_end(&mut self, f: impl FnMut(&ParsedEventGroup, &ThreadInfoState)) -> ParseResult<()> {
        // let mut total_events = 0;
        // let mut min_timestamp = u64::MAX;
        // let mut max_timestamp = 0;
        // let mut covered_dur = 0;

        info!("Waiting for encoder info...");
        loop {
            match self.packet_decoder.read_packet() {
                Ok(packet) => {
                    match packet {
                        Packet::MachineInfo(info) => {
                            if info.ver != consts::ENCODER_VERSION {
                                warn!("Encoder version mismatch! Parser: {}, Encoder: {}", consts::ENCODER_VERSION, info.ver);
                            }

                            self.machine_info = Some(info);
                        }
                        Packet::TimestampFreq(ticks_per_sec) => {
                            let ticks_per_ns = ticks_per_sec as f64 / 1_000_000_000.0;
                            info!("Got timestamp frequency: {:?} t/ns", ticks_per_ns);

                            self.ticks_per_ns = Some(ticks_per_ns);
                        }
                        Packet::DataBytes(packets) => {
                            for (header, data) in packets {
                                let thread_id = header.thread_ord_id;
                                let cur_parser_state = self.event_parsers.entry(thread_id).or_default();

                                //update thread name
                                if let Some(thread_info) = &header.thread_info {
                                    if let Some(thread_name) = thread_info.new_thread_name.clone() {
                                        cur_parser_state.thread_name = Some(thread_name);
                                        cur_parser_state.thread_id = Some(thread_info.thread_id);
                                    }
                                }

                                let mut event_buf = vec![];
                                let new_events = cur_parser_state.state_machine.decode_many(&data);
                                let new_events_len = new_events.len();
                                event_buf.extend_from_slice(&new_events);
                                debug!("Parsed {} events", new_events_len);
                                
                                cur_parser_state.state_machine.ensure_buf_end();
                                cur_parser_state.event_buf.push((header, event_buf));
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
                    }
                }
                Err(e) => {
                    warn!("Error while parsing: {:?}", e);
                    thread::sleep(Duration::from_millis(500))
                }
            }

        }

        // info!("Begin parsing... Encoder info: {:?}", encoder_info);
        // 
        // // iterate over all threads
        // for (&thread_ord_id, parser_state) in &mut self.event_parsers {
        //     let thread_name = parser_state.thread_name.clone().unwrap_or("".to_string());
        //     let thread_id = parser_state.thread_id.unwrap_or(thread_ord_id);
        //     // iterate over events
        //     for (header, events) in &parser_state.event_buf {
        //         parser_state.cur_tm = header.start_timestamp;
        //         let mut first = true;
        //         for event in events {
        //             let mut dif_tm_zero = false;
        //             if first {
        //                 first = false;
        //             }
        //             else {
        //                 let dif_tm = match event {
        //                     TracingEvent::Instant(_, dif_tm) => dif_tm,
        //                     TracingEvent::RangePart(_, dif_tm, _) => dif_tm,
        //                     TracingEvent::UnnamedRangeEnd(dif_tm, _) => dif_tm
        //                 };
        //                 if *dif_tm == 0 {
        //                     dif_tm_zero = true;
        //                 }
        //                 parser_state.cur_tm += dif_tm;
        //             }
        //             if !dif_tm_zero {
        //                 parser_state.zero_diff_cnt = 0;
        //             }
        //             else {
        //                 parser_state.zero_diff_cnt += 1;
        //             }
        //             // add to trace file
        //             let timestamp = (parser_state.cur_tm as f64 / ticks_per_ns) as u64 + parser_state.zero_diff_cnt * 10;
        //             match event {
        //                 TracingEvent::Instant(id, _) => {
        //                     let (ev_name, _) = &header.id_store.tags[*id as usize];
        //                 }
        //                 TracingEvent::RangePart(id, _, ord_id) => {
        //                     let (ev_name, ev_type) = &header.id_store.tags[*id as usize];
        //                     if let EventType::RangeEnd(start_id) = ev_type {
        //                         let (start_name, _) = &header.id_store.tags[*start_id as usize];
        //                         let start_info = parser_state.cur_started_ranges.remove(ord_id).unwrap();
        //                     }
        //                     else {
        //                         // Range start
        //                         parser_state.cur_started_ranges.insert(*ord_id, (*id, timestamp));
        //                     }
        //                 }
        //                 TracingEvent::UnnamedRangeEnd(_, ord_id ) => {
        //                     let start_info = parser_state.cur_started_ranges.remove(ord_id).unwrap();
        //                 }
        //             }
        //         }
        //         total_events += events.len();
        //         if header.start_timestamp < min_timestamp {
        //             min_timestamp = header.start_timestamp;
        //         }
        //         if header.end_timestamp > max_timestamp {
        //             max_timestamp = header.end_timestamp;
        //         }
        //         covered_dur += header.end_timestamp - header.start_timestamp;
        // 
        //     }
        // }
        
        // let ticks_per_ns = self.ticks_per_ns.unwrap_or(1.0);
        // let events_per_sec = total_events as f64 / ((max_timestamp - min_timestamp) as f64 / ticks_per_ns) * 1_000_000_000.0;
        // let events_per_sec_covered = total_events as f64 / (covered_dur as f64 / ticks_per_ns) * 1_000_000_000.0;
        // info!("Total events: {}", total_events);
        // info!("Events per second (global): {} eps", events_per_sec);
        // info!("Events per second (covered): {} eps", events_per_sec_covered);
        // info!("Average event duration: {} ns", covered_dur as f64 / ticks_per_ns / total_events as f64);
        // info!("Average bytes per event: {} bytes", self.total_event_bytes as f64 / total_events as f64);
        // info!("Average transport bytes per event: {} bytes", self.total_transport_bytes as f64 / total_events as f64);
        Ok(())
    }

    /// Continuously pull events until EOF.
    /// Decode incoming events and save them to `trace.json` in Perfetto format
    pub fn convert_to_perfetto(&mut self) -> ParseResult<BytesMut> {
        let mut trace_res_file = PerfettoTraceFile::new();
        self.parse_to_end(|group, thread_info| {
            trace_res_file.set_thread_name(thread_info.thread_id, &thread_info.thread_name);
            
            for ev in group.iter() {
                match ev {
                    ParsedEvent::Instant {
                        name,
                        tm
                    } => {
                        trace_res_file.add_point_event(&name, thread_info.thread_id, tm);
                    }
                    ParsedEvent::Range {
                        name,
                        start,
                        end
                    } => {
                        trace_res_file.add_range_event(&name, thread_info.thread_id,
                                                       start, end);
                    }
                    ParsedEvent::NamedRange {
                        name,
                        end_name,
                        start,
                        end
                    } => {

                        trace_res_file.add_range_event(&format!("{} -> {}", name, end_name), thread_info.thread_id, 
                                                       start, end);
                    }
                }
            }
        })?;
        let encoder_info = self.machine_info.take().unwrap_or_else(|| {
            warn!("Encoder info is not present in decoded data! Using default values");
            SparklesMachineInfo::default()
        });
        trace_res_file.set_process_info(encoder_info.process_name, encoder_info.pid);
        let ticks_per_ns = self.ticks_per_ns.unwrap_or_else(|| {
            warn!("Did not find timestamp frequency in decoded stream! Using default values");
            1.0
        });
        
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