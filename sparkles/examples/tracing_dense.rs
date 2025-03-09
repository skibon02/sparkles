//! Example of dense tracing conditions to test overall tracing throughput.
//! 
//! 1. Run `cargo run --example tracing_dense --release`
//! 2. Parse result file: `sparkles-parse-and-save`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

use std::hint::black_box;
use std::{env, thread};
use std::time::Instant;
use log::info;
use simple_logger::SimpleLogger;
use sparkles::config::SparklesConfig;
use sparkles_macro::{instant_event, range_event_start};

fn calc_sqrt(val: f64) -> f64 {
    val.sqrt()
}
fn perform_tracing() {
    let mut v = 0.0f64;
    
    let start = range_event_start!("perform_tracing()");
    instant_event!("k");
    instant_event!("i");
    instant_event!("t");
    instant_event!("y");
    instant_event!("d");
    instant_event!("o");
    instant_event!("g");
    for i in 0..1_000 {
        v += calc_sqrt(i as f64 + 234.532);
        instant_event!("✨");
        instant_event!("✨✨");
        instant_event!("✨✨✨");
    }
    black_box(v);
}

fn main() {
    SimpleLogger::new().init().unwrap();
    let finalize_guard = sparkles::init(
        SparklesConfig::default()
            // .with_udp_sender(38338)
    );
    
    // Only relevant if using UDP sender
    sparkles::wait_client_connected();

    let thread_count = env::args().nth(1).unwrap_or("0".to_string()).parse::<usize>().unwrap_or(0);
    let duration = env::args().nth(2).unwrap_or("100".to_string()).parse::<u64>().unwrap_or(100);
    
    info!("Launching tracing_dense example with {thread_count} additional threads and {duration} iterations");

    let mut jh_lst = vec![];
    for i in 0..thread_count {
        jh_lst.push(thread::spawn(move || {
            sparkles::set_cur_thread_name(format!("thread #{i}"));
            let g = range_event_start!("thread");
            for _ in 0..duration {
                perform_tracing();
            }
        }));
    }
    for jh in jh_lst.into_iter() {
        jh.join().unwrap();
    }

    let start = Instant::now();
    for _ in 0..duration {
        perform_tracing();
    }
    let dur = start.elapsed().as_nanos() as f64 / (duration * (3_000 + 9)) as f64;
    info!("Finished! waiting for tracer send...");
    info!("Each event took {:?} ns on average", dur);
}