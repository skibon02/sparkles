#![feature(effects)]

use std::io::{BufWriter, Write};
use std::net::UdpSocket;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use interprocess::local_socket::Stream;

pub mod thread_local_storage;
mod timestamp;
mod id_mapping;

pub fn event(hash: u32, string: &str) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.event(hash, string);
    });
}

static PACKET_NUM: AtomicUsize = AtomicUsize::new(0);

pub fn flush(writer: &mut Stream) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        let bytes = tracer.flush();

        writer.write_all(&bytes).unwrap();
        PACKET_NUM.fetch_add(1, Ordering::Relaxed);
        // println!("Packets sent: {}", PACKET_NUM.load(Ordering::Relaxed));
    });
}