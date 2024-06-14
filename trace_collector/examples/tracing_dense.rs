use std::hint::black_box;
use simple_logger::SimpleLogger;

fn calc_sqrt(val: f64) -> f64 {
    val.sqrt()
}
fn perform_tracing() {
    let mut v = 0.0f64;
    for i in 0..1_000_000 {
        v += calc_sqrt(i as f64 + 234.532);
        tracer::event(i as u8)
    }
    black_box(v);
}

fn main() {
    SimpleLogger::new().init().unwrap();
    for _ in 0..100 {
        perform_tracing();
        tracer::flush();
    }
}