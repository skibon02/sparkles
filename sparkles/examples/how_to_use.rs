
//! 1. Run `cargo run --example listen_and_print --release` in bg
//! 2. Run this example: `cargo run --example how_to_use --release`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated json file

use std::time::Duration;
use sparkles_macro::tracing_event;


fn main() {
    let jh = std::thread::Builder::new().name(String::from("thread 2")).spawn(|| {
        for _ in 0..30 {
            tracing_event!("✨✨✨");
            std::thread::sleep(Duration::from_micros(1_000));
        }

        // It is required for now, will be replaced with drop guard in future
        sparkles::flush_thread_local();
    }).unwrap();

    for _ in 0..1_000 {
        tracing_event!("✨");
        std::thread::sleep(Duration::from_micros(10));
    }

    jh.join().unwrap();

    // It is required for now, will be replaced with drop guard in future
    sparkles::finalize();
}