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
    tracing_event!("k");
    tracing_event!("i");
    tracing_event!("t");
    tracing_event!("y");
    tracing_event!("d");
    tracing_event!("o");
    for i in 0..100_000 {
        v += calc_sqrt(i as f64 + 234.532);
        tracing_event!("g");
    }
    black_box(v);
}

fn main() {
    SimpleLogger::new().init().unwrap();


    let jh1 = thread::Builder::new().name(String::from("sparkles 2")).spawn(|| {
        for _ in 0..30 {
            perform_tracing();
        }
        sparkles::flush_thread_local();
    }).unwrap();

    let jh2 = thread::Builder::new().name(String::from("sparkles 3")).spawn(|| {
        for _ in 0..30 {
            perform_tracing();
        }
        sparkles::flush_thread_local();
    }).unwrap();

    let jh3 = thread::Builder::new().name(String::from("sparkles 4")).spawn(|| {
        for _ in 0..30 {
            perform_tracing();
        }
        sparkles::flush_thread_local();
    }).unwrap();

    let jh4 = thread::Builder::new().name(String::from("sparkles 5")).spawn(|| {
        for _ in 0..30 {
            perform_tracing();
        }
        sparkles::flush_thread_local();
    }).unwrap();

    let jh5 = thread::Builder::new().name(String::from("sparkles 6")).spawn(|| {
        for _ in 0..30 {
            perform_tracing();
        }
        sparkles::flush_thread_local();
    }).unwrap();

    let jh6 = thread::Builder::new().name(String::from("sparkles 7")).spawn(|| {
        for _ in 0..30 {
            perform_tracing();
        }
        sparkles::flush_thread_local();
    }).unwrap();
    for _ in 0..100 {
        perform_tracing();
    }

    println!("Finished! waiting for tracer send...");

    sparkles::flush_thread_local();
    jh1.join().unwrap();
    jh2.join().unwrap();
    jh3.join().unwrap();
    jh4.join().unwrap();
    jh5.join().unwrap();
    jh6.join().unwrap();
    sparkles::finalize();
}