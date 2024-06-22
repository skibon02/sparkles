#![feature(effects)]

use std::net::UdpSocket;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;

pub mod thread_local_storage;
mod timestamp;
mod id_mapping;

pub fn event(hash: u32, string: &str) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.event(hash, string);
    });
}

pub fn flush(tx: &mut Sender<Box<[u8]>>) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        let bytes = tracer.flush();
        //split bytes into chunks of 1024 bytes
        tx.send(bytes).unwrap();
        // println!("Packets sent: {}", PACKET_NUM.load(Ordering::Relaxed));
    });
}