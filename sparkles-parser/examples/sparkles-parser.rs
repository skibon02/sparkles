use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use log::{error, info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles_parser::{TRACE_RESULT_FILE, TraceAcceptor};

fn save_res_and_exit() {
    //save as trace.json
    let trace_data = TRACE_RESULT_FILE.lock().unwrap();
    let events_cnt = trace_data.trace_events.len();
    if events_cnt > 5_000_000 {
        error!("you dumbass really want to save {} events to your hard drive? fuck you!", events_cnt);
        std::process::exit(0);
    }
    info!("Events count: {}. Saving to trace.json...", events_cnt);
    let trace_data = serde_json::to_string(&*trace_data).unwrap();
    std::fs::write("trace.json", trace_data).unwrap();

    std::process::exit(0);
}

fn main() {

    static IS_EXITING: AtomicBool = AtomicBool::new(false);
    ctrlc::set_handler(move || {
        thread::spawn(|| {
            info!("Received Ctrl+C!");

            if IS_EXITING.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).is_err() {
                return;
            }
            save_res_and_exit();
        });
    }).unwrap();

    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();
    TraceAcceptor::default().listen().unwrap();
    if IS_EXITING.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
        save_res_and_exit();
    }
}