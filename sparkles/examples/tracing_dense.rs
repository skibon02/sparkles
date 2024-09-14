use std::hint::black_box;
use std::time::Instant;
use log::info;
use simple_logger::SimpleLogger;
use sparkles::SparklesConfigBuilder;
use sparkles_macro::tracing_event;

fn calc_sqrt(val: f64) -> f64 {
    val.sqrt()
}
fn perform_tracing() {
    let mut v = 0.0f64;
    tracing_event!("k");
    tracing_event!("i");
    tracing_event!("t");
    tracing_event!("y");
    tracing_event!("d");
    tracing_event!("o");
    tracing_event!("g");
    for i in 0..1_000 {
        v += calc_sqrt(i as f64 + 234.532);
        tracing_event!("✨");
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