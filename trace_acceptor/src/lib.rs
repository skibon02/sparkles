mod perfetto_format;

use std::{mem, thread};
use std::net::UdpSocket;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use interprocess::local_socket::traits::ListenerExt;
use lazy_static::lazy_static;
use log::{debug, error, info};
use crate::perfetto_format::PerfettoTraceFile;

pub struct TraceAcceptor {
    stream_parser: ParsingStateMachine,
}

const MEASURE_DUR_NS: usize = 19;

lazy_static! {
    pub static ref TRACE_RESULT_FILE: Mutex<PerfettoTraceFile> = Mutex::new(PerfettoTraceFile::new());
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum TracingEventId {
    MainLoopStart,
    MainLoopEnd,

    DriversPoll,

    PlcLogicIterStart,
    PlcLogicIterEnd,

    I2cExpanderDriverPoll,
    DebugConnectionDriverNewCmd,
    Rs485DriverPoll,

    IomGetStart,
    PlcPollStart,
    IomSetStart,
    RetainOperationStart,

    I2CWriteOperationStart,
    I2CWriteOperationEnd,
    I2CWriteOperationEndErr,

    I2CReadOperationStart,
    I2CReadOperationEnd,
    I2CReadOperationEndErr,

    I2CWakerCall,
    I2CWakerCallErr,

    SpiOpStart,
    SpiOpFail,
    SpiOpEnd,

    DmaOpStart,
    DmaOpEnd,
    DmaOpEndErr,

    DmaWakerCall,
    DmaPollFn,

    Unknown, // 28
}

impl From<u8> for TracingEventId {
    fn from(val: u8) -> Self {
        match val {
            0..28 => unsafe { mem::transmute(val) },
            _ => Unknown,
        }
    }
}
use TracingEventId::*;

impl TraceAcceptor {
    pub fn new() -> Self {
        Self {
            stream_parser: ParsingStateMachine::EventId
        }
    }

    pub fn listen(&mut self)  {
        // let udp_socket = UdpSocket::bind("0.0.0.0:4302").unwrap();

        let mut total_pr = 0;
        let mut first = true;
        info!("Listening for incoming packets...");

        {
            let mut trace_res_file = TRACE_RESULT_FILE.lock().unwrap();
            trace_res_file.set_thread_name(MainLoopStart as u8, format!("{:?}", MainLoopStart));
            trace_res_file.set_thread_name(MainLoopEnd as u8, format!("{:?}", MainLoopEnd));
            trace_res_file.set_thread_name(DriversPoll as u8, format!("{:?}", DriversPoll));
            trace_res_file.set_thread_name(PlcLogicIterStart as u8, format!("{:?}", PlcLogicIterStart));
            trace_res_file.set_thread_name(PlcLogicIterEnd as u8, format!("{:?}", PlcLogicIterEnd));
            trace_res_file.set_thread_name(I2cExpanderDriverPoll as u8, format!("{:?}", I2cExpanderDriverPoll));
            trace_res_file.set_thread_name(DebugConnectionDriverNewCmd as u8, format!("{:?}", DebugConnectionDriverNewCmd));
            trace_res_file.set_thread_name(Rs485DriverPoll as u8, format!("{:?}", Rs485DriverPoll));
            trace_res_file.set_thread_name(IomGetStart as u8, format!("{:?}", IomGetStart));
            trace_res_file.set_thread_name(PlcPollStart as u8, format!("{:?}", PlcPollStart));
            trace_res_file.set_thread_name(IomSetStart as u8, format!("{:?}", IomSetStart));
            trace_res_file.set_thread_name(RetainOperationStart as u8, format!("{:?}", RetainOperationStart));
            trace_res_file.set_thread_name(I2CWriteOperationStart as u8, format!("{:?}", I2CWriteOperationStart));
            trace_res_file.set_thread_name(I2CWriteOperationEnd as u8, format!("{:?}", I2CWriteOperationEnd));
            trace_res_file.set_thread_name(I2CWriteOperationEndErr as u8, format!("{:?}", I2CWriteOperationEndErr));
            trace_res_file.set_thread_name(I2CReadOperationStart as u8, format!("{:?}", I2CReadOperationStart));
            trace_res_file.set_thread_name(I2CReadOperationEnd as u8, format!("{:?}", I2CReadOperationEnd));
            trace_res_file.set_thread_name(I2CReadOperationEndErr as u8, format!("{:?}", I2CReadOperationEndErr));
            trace_res_file.set_thread_name(I2CWakerCall as u8, format!("{:?}", I2CWakerCall));
            trace_res_file.set_thread_name(I2CWakerCallErr as u8, format!("{:?}", I2CWakerCallErr));
            trace_res_file.set_thread_name(SpiOpStart as u8, format!("{:?}", SpiOpStart));
            trace_res_file.set_thread_name(SpiOpFail as u8, format!("{:?}", SpiOpFail));
            trace_res_file.set_thread_name(SpiOpEnd as u8, format!("{:?}", SpiOpEnd));
            trace_res_file.set_thread_name(DmaOpStart as u8, format!("{:?}", DmaOpStart));
            trace_res_file.set_thread_name(DmaOpEnd as u8, format!("{:?}", DmaOpEnd));
            trace_res_file.set_thread_name(DmaOpEndErr as u8, format!("{:?}", DmaOpEndErr));
            trace_res_file.set_thread_name(DmaWakerCall as u8, format!("{:?}", DmaWakerCall));
            trace_res_file.set_thread_name(DmaPollFn as u8, format!("{:?}", DmaPollFn));
        }


        use interprocess::local_socket::{prelude::*, GenericNamespaced, ListenerOptions, Stream};
        use std::io::{self, prelude::*, BufReader};

        let printname = "tracer.sock";
        let name = printname.to_ns_name::<GenericNamespaced>().unwrap();
        let opts = ListenerOptions::new().name(name);

        let listener = match opts.create_sync() {
            Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
                error!(
                    "Error: could not start server because the socket file is occupied. Please check if
				{printname} is in use by another process and try again."
                );
                return;
            }
            x => x.unwrap(),
        };
        info!("Server running at {printname}");
        info!("Waiting for connection...");
        let mut buf = vec![0; 1_000_000];
        let mut con = listener.incoming().next().unwrap().unwrap();
        // let mut reader = BufReader::with_capacity(5_000_000, con);
        info!("Client connected!");

        let start = Instant::now();
        let mut last_sec_print = 0;
        let mut bytes_cnt = 0;
        let mut events_cnt = 0;
        let mut packets_cnt = 0;

        let mut events = Vec::with_capacity(10_000_000);
        loop {
            let c = con.read(&mut buf).unwrap();
            if c == 0 {
                info!("Client disconnected! Exiting...");
                break;
            }
            let new_events = self.stream_parser.parse_many(&buf[..c]);
            let new_events_len = new_events.len();
            events.extend(new_events);

            debug!("Got {} bytes, Parsed {} events", c, new_events_len);
            bytes_cnt += c;
            events_cnt += new_events_len;
            packets_cnt += 1;

            if start.elapsed().as_secs() > last_sec_print {
                last_sec_print = start.elapsed().as_secs();

                info!("");
                info!("Packets per second: {}", packets_cnt);
                info!("Bytes per second: {}", bytes_cnt);
                info!("Events per second: {}", events_cnt);
                info!("Avg bytes per event: {}", bytes_cnt as f64 / events_cnt as f64);
                let ovh_ms = events_cnt * MEASURE_DUR_NS / 1_000;
                info!("Total measuring overhead: {}us per second ({}%)", ovh_ms, ovh_ms as f64 / 1_000_000.0 * 100.0);
                bytes_cnt = 0;
                events_cnt = 0;
                packets_cnt = 0;
            }
        }

        info!("Disconnected... Start parsing");

        for event in events {
            if first {
                first = false;
            }
            else {
                total_pr += event.2;
            }
            // add to trace file
            let mut trace_res_file = TRACE_RESULT_FILE.lock().unwrap();
            trace_res_file.add_point_event(format!("{:?}", event.0), event.0 as u8, ((event.1 as u64 | (total_pr << 16)) as f64 / 2.495) as u64);
        }

        info!("Total PR: {}", total_pr);

        info!("Finished!");

    }
}

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