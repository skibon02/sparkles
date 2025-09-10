use std::arch::x86_64::_rdtsc;
use std::time::{Instant};
use log::info;
use simple_logger::SimpleLogger;
use sparkles_core::TimestampProvider;

fn main() {
    SimpleLogger::new().init().unwrap();

    let now = sparkles_core::Timestamp::now();
    let start = Instant::now();

    while start.elapsed().as_secs() < 1 {}
    let end = sparkles_core::Timestamp::now();
    info!("Your CPU timestamps frequency: {:.3}GHz", end.wrapping_sub(now) as f64 / 1_000_000_000.0);
}