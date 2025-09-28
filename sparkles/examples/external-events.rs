//! Example of using external events.
//!
//! 1. Run `cargo run --example external-events --release`
//! 2. Parse result file: `sparkles-parse-and-save`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

#[path = "../examples_common.rs"]
pub mod common;

use std::{thread};
use std::time::{Duration};
use clap::Parser;
use log::{info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles::{instant_event, range_event_start};
use sparkles::external_events::ExternalEventsSource;
use sparkles::monotonic::get_monotonic;
use sparkles_core::{Timestamp, TimestampProvider};
use sparkles_macro::static_name;
use crate::common::Args;


fn main() {
    SimpleLogger::default().with_level(LevelFilter::Info).init().unwrap();

    let args = Args::parse();
    info!("Running with args: {args:?}");
    let cfg = args.sparkles_cfg();
    let finalize_guard = sparkles::init(cfg);

    // Only relevant if using UDP sender
    sparkles::wait_client_connected();

    let g = range_event_start!("main");
    let mut external_events = ExternalEventsSource::new("GPU".to_string());

    // sync time
    let now = Timestamp::now();
    let host_now = get_monotonic();
    thread::sleep(Duration::from_millis(10));
    let now2 = Timestamp::now();
    let host_now2 = get_monotonic();

    external_events.push_sync_point(host_now, now);
    external_events.push_sync_point(host_now2, now2);

    // simulate some GPU events
    let update_start = Timestamp::now();
    thread::sleep(Duration::from_millis(10));
    let render_start = Timestamp::now();
    thread::sleep(Duration::from_millis(30));
    let render_end = Timestamp::now();
    thread::sleep(Duration::from_millis(1));
    let finished = Timestamp::now();

    instant_event!("Recording external events");
    let update_name = external_events.map_event_name(static_name!("GPU Update"));
    let render_name = external_events.map_event_name(static_name!("GPU Render"));
    let finished_name = external_events.map_event_name(static_name!("finished!"));
    external_events.push_events(&[update_start, render_start, render_start, render_end, finished], &[(update_name, 0x01), (update_name, 0x81),
        (render_name, 0x01), (render_name, 0x81), (finished_name, 0x00)]);
    
    thread::sleep(Duration::from_millis(1));
}