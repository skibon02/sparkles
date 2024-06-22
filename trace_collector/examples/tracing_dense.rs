use std::hash::Hash;
use std::hint::black_box;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::{mem, thread};
use std::time::Duration;
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
    for i in 0..10_000 {
        v += calc_sqrt(i as f64 + 234.532);
        tracer::event(id_map!("haha"), "haha");
    }
    black_box(v);
}

static PACKET_NUM: AtomicUsize = AtomicUsize::new(0);

fn main() {
    SimpleLogger::new().init().unwrap();

    let udp_socket = UdpSocket::bind("0.0.0.0:4303").unwrap();
    udp_socket.connect("127.0.0.1:4302").unwrap();

    let (mut tx, rx) = channel::<Box<[u8]>>();
    let sending_thread = thread::spawn(move || {
        while let Ok(bytes) = rx.recv() {
            let mut chunks = bytes.chunks(1500);
            let mut cnt = 0;
            for chunk in chunks {
                udp_socket.send(chunk).unwrap();
                cnt += 1;
                thread::sleep(Duration::from_micros(3));
            }
            PACKET_NUM.fetch_add(cnt, Ordering::Relaxed);
        }
    });

    for _ in 0..5_000 {
        perform_tracing();
        tracer::flush(&mut tx);
    }

    mem::drop(tx);

    println!("Finished! waiting for tracer send...");
    sending_thread.join().unwrap();
}