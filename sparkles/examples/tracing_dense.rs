use std::hint::black_box;
use std::time::Instant;
use log::info;
use simple_logger::SimpleLogger;
use sparkles::SparklesConfigBuilder;
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
    }
    black_box(v);
}

fn main() {
    SimpleLogger::new().init().unwrap();
    let finalize_guard = SparklesConfigBuilder::default_init();

    let start = Instant::now();
    for _ in 0..100 {
        perform_tracing();
    }

    let dur = start.elapsed().as_nanos() as f64 / (100 * (1_000 + 6)) as f64;
    info!("Finished! waiting for tracer send...");
    info!("Each event took {:?} ns", dur);
}