//! Configuration example
//! 1. Run `cargo run --example configuration --release`
//! 2. Parse result file: `cargo run --release --example single_file trace.sprk`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

use log::LevelFilter;
use simple_logger::SimpleLogger;
use sparkles::config::SparklesConfig;
use sparkles::sender::file_sender::FileSenderConfig;
use sparkles_macro::{instant_event, range_event_start};

fn main() {
    SimpleLogger::default().with_level(LevelFilter::Debug).init().unwrap();
    // `SparklesConfig` is a builder, you can easily add configuration using .with() chain.
    let config = SparklesConfig::default()
        // Provide custom name
        .with_file_sender_config(FileSenderConfig {
            output_filename: Some("trace.sprk".to_string())
        })
        // Increase thread-local flush threshold, so flushing to global storage will be less frequent
        .with_thread_flush_attempt_threshold(100_000);
    
    let finalize_guard = sparkles::init(config);
    let g = range_event_start!("main()");

    // We expect to have ~3 flushes as single event in dense tracing conditions is 3 bytes long
    for _ in 0..50_000 {
        instant_event!("✨✨✨");
    }
}