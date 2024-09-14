
//! 1. Run `cargo run --example listen_and_print --release` in bg
//! 2. Run this example: `cargo run --example how_to_use --release`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated json file

use std::thread;
use std::time::Duration;
use log::LevelFilter;
use simple_logger::SimpleLogger;
use sparkles::SparklesConfigBuilder;
use sparkles_macro::tracing_event;

fn main() {
    SimpleLogger::default().with_level(LevelFilter::Debug).init().unwrap();
    // Init and acquire finalize guard to automatically finalize event collection and 
    // flush them to the destination when the main thread finished
    let finalize_guard = SparklesConfigBuilder::default_init();

    // Flushing: Events are preserved because this thread is joined later in code
    let jh = thread::Builder::new().name(String::from("joined thread")).spawn(|| {
        for _ in 0..100 {
            tracing_event!("^-^");
            thread::sleep(Duration::from_micros(1_000));
        }
    }).unwrap();

    // Flushing: Flushing for threads that are not joined is not guaranteed, please use `sparkles::flush_thread_local()`;
    thread::Builder::new().name(String::from("detached thread")).spawn(|| {
        for _ in 0..30 {
            tracing_event!("*_*");
            thread::sleep(Duration::from_micros(1_000));
        }
        // sparkles::flush_thread_local();
    }).unwrap();

    for _ in 0..1_000 {
        tracing_event!("✨✨✨");
        thread::sleep(Duration::from_micros(10));
    }

    // In case of panic, main thread preserves captured events because of drop guard
    // panic!("BOOM");

    //Thread join automatically flush trace data to the global storage
    jh.join().unwrap();

    // <- execution of `drop` for `finalize_guard`
}