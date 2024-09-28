mod perfetto_format;
mod consts;
mod decoder;

use std::cmp::min;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use log::{debug, error, info, warn};
use thiserror::Error;
use sparkles_core::headers::{LocalPacketHeader, SparklesEncoderInfo};
use sparkles_core::local_storage::id_mapping::EventType;
use crate::decoder::StreamFrameDecoder;
use crate::ParseError::Decode;
use crate::perfetto_format::PerfettoTraceFile;

pub static PARSER_BUF_SIZE: usize = 1_000_000;

#[derive(Default)]
pub struct SparklesParser {
    total_event_bytes: u64,
    total_transport_bytes: u64,

    encoder_info: Option<SparklesEncoderInfo>,
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

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Error while decoding frame")]
    Decode(DecodeError),
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("Error while reading from stream")]
    Io(#[from] std::io::Error),
    #[error("Error while deserializing data")]
    Bincode(#[from] bincode::Error),
}

type ParseResult<T> = Result<T, ParseError>;
type DecodeResult<T> = Result<T, DecodeError>;

impl SparklesParser {
    /// Decode incoming events and save them to `trace.json` in Perfetto format
    pub fn parse_and_save(&mut self, mut reader: impl Read) -> ParseResult<()> {
        if let Err(e) = self.decode_packets(&mut reader) {
            error!("Error handling client: {:?}", e);
            return Err(Decode(e));
        }

        //some stats
        let mut total_events = 0;
        let mut min_timestamp = u64::MAX;
        let mut max_timestamp = 0;
        let mut covered_dur = 0;


        let encoder_info = self.encoder_info.take().unwrap_or_else(|| {
            warn!("Encoder info is not present in decoded data! Using default values");
            SparklesEncoderInfo::default()
        });

        info!("Begin parsing... Encoder info: {:?}", encoder_info);

        let mut trace_res_file = PerfettoTraceFile::new(encoder_info.process_name, encoder_info.pid);
        let ticks_per_ns = self.ticks_per_ns.unwrap_or_else( || {
            warn!("Did not find timestamp frequency in decoded stream! Using default values");
            1.0
        });
        // iterate over all threads
        for (&thread_ord_id, parser_state) in &mut self.event_parsers {
            let thread_name = parser_state.thread_name.clone().unwrap_or("".to_string());
            let thread_id = parser_state.thread_id.unwrap_or(thread_ord_id);
            // iterate over events
            for (header, events) in &parser_state.event_buf {
                trace_res_file.set_thread_name(thread_id, thread_name.clone());

                parser_state.cur_tm = header.start_timestamp;
                let mut first = true;
                for event in events {
                    let mut dif_tm_zero = false;
                    if first {
                        first = false;
                    }
                    else {
                        let dif_tm = match event {
                            TracingEvent::Instant(_, dif_tm) => dif_tm,
                            TracingEvent::RangePart(_, dif_tm, _) => dif_tm,
                            TracingEvent::UnnamedRangeEnd(dif_tm, _) => dif_tm
                        };
                        if *dif_tm == 0 {
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
                    // add to trace file
                    let timestamp = (parser_state.cur_tm as f64 / ticks_per_ns) as u64 + parser_state.zero_diff_cnt * 10;
                    match event {
                        TracingEvent::Instant(id, _) => {
                            let (ev_name, _) = &header.id_store.tags[*id as usize];
                            trace_res_file.add_point_event(ev_name.clone(), thread_id, timestamp);
                        }
                        TracingEvent::RangePart(id, _, ord_id) => {
                            let (ev_name, ev_type) = &header.id_store.tags[*id as usize];
                            if let EventType::RangeEnd(start_id) = ev_type {
                                let (start_name, _) = &header.id_store.tags[*start_id as usize];
                                let start_info = parser_state.cur_started_ranges.remove(ord_id).unwrap();
                                let start_tm = start_info.1;
                                let end_tm = timestamp;
                                trace_res_file.add_range_event(format!("{} -> {}", start_name, ev_name), thread_id, start_tm, end_tm);
                            }
                            else {
                                // Range start
                                parser_state.cur_started_ranges.insert(*ord_id, (*id, timestamp));
                            }
                        }
                        TracingEvent::UnnamedRangeEnd(_, ord_id ) => {
                            let start_info = parser_state.cur_started_ranges.remove(ord_id).unwrap();
                            let range_id = start_info.0;
                            let range_name = &header.id_store.tags[range_id as usize].0;
                            let start_tm = start_info.1;
                            let end_tm = timestamp;
                            trace_res_file.add_range_event(range_name.clone(), thread_id, start_tm, end_tm);
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

        let events_per_sec = total_events as f64 / ((max_timestamp - min_timestamp) as f64 / ticks_per_ns) * 1_000_000_000.0;
        let events_per_sec_covered = total_events as f64 / (covered_dur as f64 / ticks_per_ns) * 1_000_000_000.0;
        info!("Total events: {}", total_events);
        info!("Events per second (global): {} eps", events_per_sec);
        info!("Events per second (covered): {} eps", events_per_sec_covered);
        info!("Average event duration: {} ns", covered_dur as f64 / ticks_per_ns / total_events as f64);
        info!("Average bytes per event: {} bytes", self.total_event_bytes as f64 / total_events as f64);
        info!("Average transport bytes per event: {} bytes", self.total_transport_bytes as f64 / total_events as f64);

        info!("Finished! Saving to trace.perf...");

        let mut file = std::fs::File::create("trace.perf").unwrap();
        let bytes = trace_res_file.get_bytes();
        file.write_all(&bytes).unwrap();

        Ok(())
    }

    fn decode_packets(&mut self, con: &mut impl Read) -> DecodeResult<()> {
        loop {
            let mut packet_type = [0u8; 1];
            con.read_exact(&mut packet_type)?;
            info!("Packet id: {}", packet_type[0]);

            let mut events_bytes = vec![0; 10_000];

            match packet_type[0] {
                0x00 => {
                    let mut info_bytes_len = [0u8; 8];
                    con.read_exact(&mut info_bytes_len)?;
                    let info_bytes_len = u64::from_le_bytes(info_bytes_len) as usize;

                    let mut info_bytes = vec![0u8; info_bytes_len];
                    con.read_exact(&mut info_bytes)?;
                    let info = bincode::deserialize::<SparklesEncoderInfo>(&info_bytes)?;

                    if info.ver != consts::ENCODER_VERSION {
                        warn!("Encoder version mismatch! Parser: {}, Encoder: {}", consts::ENCODER_VERSION, info.ver);
                    }

                    self.encoder_info = Some(info);
                }
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
                        let header = bincode::deserialize::<LocalPacketHeader>(&header_bytes)?;

                        let mut buf_len = [0u8; 8];
                        con.read_exact(&mut buf_len)?;
                        self.total_transport_bytes += 8;
                        let buf_len = u64::from_le_bytes(buf_len) as usize;

                        let mut event_buf = Vec::with_capacity(PARSER_BUF_SIZE);
                        info!("Got packet header: {:?}", header);

                        let thread_id = header.thread_ord_id;
                        let cur_parser_state = self.event_parsers.entry(thread_id).or_default();

                        //update thread name
                        if let Some(thread_info) = &header.thread_info {
                            if let Some(thread_name) = thread_info.new_thread_name.clone() {
                                cur_parser_state.thread_name = Some(thread_name);
                                cur_parser_state.thread_id = Some(thread_info.thread_id);
                            }
                        }

                        let mut remaining_size = buf_len;
                        while remaining_size > 0 {
                            let cur_size = min(PARSER_BUF_SIZE, remaining_size);
                            events_bytes.resize(cur_size, 0);
                            con.read_exact(&mut events_bytes)?;
                            self.total_transport_bytes += cur_size as u64;

                            let new_events = cur_parser_state.state_machine.decode_many(&events_bytes);
                            let new_events_len = new_events.len();
                            event_buf.extend_from_slice(&new_events);
                            debug!("Got {} bytes, Parsed {} events", cur_size, new_events_len);
                            self.total_event_bytes += cur_size as u64;

                            remaining_size -= cur_size;
                        }
                        cur_parser_state.state_machine.ensure_buf_end();

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
                    let header = bincode::deserialize::<LocalPacketHeader>(&header_bytes)?;

                    info!("Got failed packet header: {:?}", header);

                    let start = header.start_timestamp;
                    let dur = header.end_timestamp - header.start_timestamp;
                    let thread_ord_id = header.thread_ord_id;
                    self.thread_parser_state(thread_ord_id).missed_events.push((start, dur));

                },
                0x03 => {
                    let mut bytes = [0u8; 8];
                    con.read_exact(&mut bytes)?;
                    let ticks_per_sec = u64::from_le_bytes(bytes);
                    let ticks_per_ns = ticks_per_sec as f64 / 1_000_000_000.0;
                    info!("Got timestamp frequency: {:?} t/ns", ticks_per_ns);

                    self.ticks_per_ns = Some(ticks_per_ns);
                }
                0xff => {
                    info!("Client was gracefully disconnected!");

                    return Ok(());
                }
                _ => panic!("Unknown packet type!")
            }
        }
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