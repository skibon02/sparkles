//! Tracing long example
//! 1. Run `cargo run --example tracing_long --release`
//! 2. Parse result file: `sparkles-parse-and-save`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

use std::hint::black_box;
use std::thread;
use std::time::{Duration, Instant};
use log::info;
use simple_logger::SimpleLogger;
use sparkles::config::SparklesConfig;
use sparkles_macro::{instant_event, range_event_start};

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
    thread::sleep(Duration::from_millis(1));
    black_box(v);
}

fn main() {
    SimpleLogger::new().init().unwrap();
    let finalize_guard = sparkles::init(SparklesConfig::default()
        // .with_default_udp_sender()
        .with_flush_threshold(0.003));
    
    sparkles::wait_client_connected();

    let start = Instant::now();
    let t1 = thread::spawn(|| {
        sparkles::set_cur_thread_name("thread#2".to_string());
        let g = range_event_start!("thread#2");
        for _ in 0..500 {
            perform_tracing();
        }
    });
    let t2 = thread::spawn(|| {
        sparkles::set_cur_thread_name("thread#3".to_string());
        let g = range_event_start!("thread#3");
        for _ in 0..500 {
            perform_tracing();
        }
    });
    let t3 = thread::spawn(|| {
        sparkles::set_cur_thread_name("thread#4".to_string());
        let g = range_event_start!("thread#4");
        for _ in 0..500 {
            perform_tracing();
        }
    });
    for _ in 0..500 {
        perform_tracing();
    }

    let dur = start.elapsed().as_nanos() as f64 / (100 * (3_000 + 9)) as f64;
    info!("Finished! waiting for tracer send...");
    info!("Each event took {:?} ns", dur);

    t1.join().unwrap();
    t2.join().unwrap();
    t3.join().unwrap();
}