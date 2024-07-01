use std::hash::Hash;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{mem, thread};
use std::sync::mpsc::channel;
use std::hint::black_box;
use std::io::BufWriter;
use interprocess::local_socket::ToNsName;
use simple_logger::SimpleLogger;
use tracer_macro::tracing_event;

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
    for i in 0..100_000 {
        v += calc_sqrt(i as f64 + 234.532);
        tracing_event!("haha");
    }
    black_box(v);
}

fn main() {
    SimpleLogger::new().init().unwrap();

    use interprocess::local_socket::{prelude::*, GenericNamespaced, Stream};
    use std::io::prelude::*;

    let name = "tracer.sock";
    let mut conn = Stream::connect(name.to_ns_name::<GenericNamespaced>().unwrap()).unwrap();
    // let mut writer = BufWriter::with_capacity(10_000, conn);


    for _ in 0..10_000 {
        perform_tracing();
        // let start = Instant::now();

        tracer::flush(&mut conn);
        // info!("Time taken to send all packets: {:?}", start.elapsed());
    }

    println!("Finished! waiting for tracer send...");
}