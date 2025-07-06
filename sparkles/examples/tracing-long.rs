//! Tracing long example
//! 
//! 1. Run `cargo run --example tracing_long --release`
//! 2. Parse result file: `sparkles-parse-and-save`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

#[path = "../examples_common.rs"]
pub mod common;

use std::hint::black_box;
use std::{env, thread};
use std::time::Duration;
use clap::Parser;
use log::{info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles_macro::{instant_event, range_event_start};
use crate::common::Args;

fn calc_sqrt(val: f64) -> f64 {
    val.sqrt()
}
fn perform_tracing() {
    let mut v = 0.0f64;
    
    let start = range_event_start!("outer");
    instant_event!("k");
    instant_event!("i");
    instant_event!("t");
    instant_event!("y");
    instant_event!("d");
    instant_event!("o");
    instant_event!("g");
    for i in 0..100 {
        let start = range_event_start!("inner");
        let start = range_event_start!("inner2");
        let start = range_event_start!("inner3");
        let start = range_event_start!("inner4");
        let start = range_event_start!("inner5");
        v += calc_sqrt(i as f64 + 234.532);
        instant_event!("✨");
        instant_event!("✨✨");
        instant_event!("✨✨✨");
    }
    thread::sleep(Duration::from_millis(10));
    black_box(v);
}

fn main() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();

    let args = Args::parse();
    info!("Running with args: {args:?}");
    let cfg = args.sparkles_cfg();
    let finalize_guard = sparkles::init(cfg
        .with_flush_threshold(32000)
        .with_thread_flush_attempt_threshold(16000)
    );
    
    // Only relevant if using UDP sender
    sparkles::wait_client_connected();

    // Arguments
    let duration_s = env::args().nth(1).unwrap_or("10".to_string()).parse::<u64>().unwrap_or(10);
    
    info!("Begin generating trace data for {duration_s}s");
    for _ in 0..duration_s * 100 {
        perform_tracing();
    }
    info!("Done!");
}