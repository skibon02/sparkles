use std::thread;
use log::LevelFilter;
use simple_logger::SimpleLogger;
use trace_acceptor::{TRACE_RESULT_FILE, TraceAcceptor};



fn main() {

    ctrlc::set_handler(move || {
        thread::spawn(|| {
            println!("Received Ctrl+C!");

            //save as trace.json
            let trace_data = TRACE_RESULT_FILE.lock().unwrap();
            let trace_data = serde_json::to_string(&*trace_data).unwrap();
            std::fs::write("trace.json", trace_data).unwrap();

            std::process::exit(0);
        });
    }).unwrap();

    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();
    TraceAcceptor::new().listen();
}