use std::hash::Hash;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{mem, thread};
use std::sync::mpsc::channel;
use std::hint::black_box;
use std::io::BufWriter;
use interprocess::local_socket::ToNsName;
use simple_logger::SimpleLogger;
use tracer_macro::id_map;

fn calc_sqrt(val: f64) -> f64 {
    val.sqrt()
}
fn perform_tracing() {
    let mut v = 0.0f64;
    tracer::event(id_map!("meow"), "meow");
    tracer::event(id_map!("meow1"), "meow");
    tracer::event(id_map!("meow2"), "meow");
    tracer::event(id_map!("meow3"), "meow");
    tracer::event(id_map!("meow4"), "meow");
    tracer::event(id_map!("meow5"), "meow");
    for i in 0..100_000 {
        v += calc_sqrt(i as f64 + 234.532);
        tracer::event(id_map!("haha"), "haha");
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