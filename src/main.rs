#![feature(generic_const_exprs)]

use std::sync::Arc;
use std::thread;
use log::info;
use simple_logger::SimpleLogger;
use crate::tracing::{IS_FINISHED, Tracer};

mod tracing;
mod fifo;

fn main() {
    SimpleLogger::new().init().unwrap();
    let tracer = Arc::new(Tracer::new());
    info!("Started!");

    let h1 = {
        let tracer = tracer.clone();
        thread::spawn(move || {
            for i in 0..30_000_000 {
                tracer.event(1);
            }
        })
    };
    // let h2 = {
    //     let tracer = tracer.clone();
    //     thread::spawn(move || {
    //         for i in 0..10_000_000 {
    //             tracer.event(2);
    //         }
    //     })
    // };
    // let h3 = {
    //     let tracer = tracer.clone();
    //     thread::spawn(move || {
    //         for i in 0..10_000_000 {
    //             tracer.event(3);
    //         }
    //     })
    // };
    // let h4 = {
    //     let tracer = tracer.clone();
    //     thread::spawn(move || {
    //         for i in 0..100_000_000 {
    //             tracer.event(4);
    //         }
    //     })
    // };
    // let h5 = {
    //     let tracer = tracer.clone();
    //     thread::spawn(move || {
    //         for i in 0..100_000_000 {
    //             tracer.event(5);
    //         }
    //     })
    // };

    h1.join().unwrap();
    // h2.join().unwrap();
    // h3.join().unwrap();
    // h4.join().unwrap();
    // h5.join().unwrap();

    IS_FINISHED.store(true, std::sync::atomic::Ordering::Relaxed);
}