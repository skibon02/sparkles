use std::hash::Hash;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{mem, thread};
use std::sync::mpsc::channel;
use std::hint::black_box;
use std::io::BufWriter;
use std::time::Duration;
use interprocess::local_socket::ToNsName;
use simple_logger::SimpleLogger;
use sparkles_macro::tracing_event;

fn calc_sqrt(val: f64) -> f64 {
    val.sqrt()
}
fn perform_tracing() {
    let mut v = 0.0f64;
    tracing_event!("meow");
    tracing_event!("meow1");
    tracing_event!("meow2");
    tracing_event!("meow3");
    tracing_event!("meow4");
    tracing_event!("meow5");
    for i in 0..1_000 {
        v += calc_sqrt(i as f64 + 234.532);
        tracing_event!("haha");
    }
    black_box(v);
}

fn main() {
    SimpleLogger::new().init().unwrap();


    thread::Builder::new().name(String::from("another thread")).spawn(|| {
        for _ in 0..10 {
            perform_tracing();
        }
        sparkles::flush_thread_local();
    }).unwrap();
    for _ in 0..10 {
        perform_tracing();
    }

    println!("Finished! waiting for tracer send...");

    sparkles::flush_thread_local();
    sparkles::finalize();
}