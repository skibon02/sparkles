//! How to use example
//! 1. Run `cargo run --example how_to_use --release`
//! 2. Parse result file: `cargo run --release --example interactive`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

use std::thread;
use std::time::Duration;
use log::LevelFilter;
use simple_logger::SimpleLogger;
use sparkles_macro::{instant_event, range_event_end, range_event_start};

fn main() {
    SimpleLogger::default().with_level(LevelFilter::Debug).init().unwrap();
    // Init and acquire finalize guard to automatically finalize event collection and 
    // flush them to the destination when the main thread finished
    let finalize_guard = sparkles::init_default();
    // Start range event
    // It's finished when guard is dropped
    let g = range_event_start!("main()");

    // Flushing: Events are preserved because this thread is joined later in code
    let jh = thread::Builder::new().name(String::from("joined thread")).spawn(|| {
        let g = range_event_start!("joined thread");
        for _ in 0..100 {
            instant_event!("^-^");
            thread::sleep(Duration::from_micros(1_000));
        }
    }).unwrap();

    // Flushing: Flushing for threads that are not joined is not guaranteed, please use `sparkles::flush_thread_local()`;
    thread::Builder::new().name(String::from("detached thread")).spawn(|| {
        let g = range_event_start!("detached thread");
        for _ in 0..30 {
            instant_event!("*_*");
            thread::sleep(Duration::from_micros(1_000));
        }
        range_event_end!(g, "custom range end message!");
        // sparkles::flush_thread_local();
    }).unwrap();

    for _ in 0..1_000 {
        instant_event!("✨✨✨");
        thread::sleep(Duration::from_micros(10));
    }

    // In case of panic, main thread preserves captured events because of drop guard
    // panic!("BOOM");

    //Thread join automatically flush trace data to the global storage
    jh.join().unwrap();

    // <- execution of `drop` for `finalize_guard`
}