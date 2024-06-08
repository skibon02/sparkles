#![feature(generic_const_exprs)]

use std::sync::Arc;
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use log::info;
use simple_logger::SimpleLogger;
use crate::tracing::{IS_FINISHED, Tracer};

pub mod tracing;
mod fifo;

fn run() {

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

    h1.join().unwrap();

    IS_FINISHED.store(true, std::sync::atomic::Ordering::Relaxed);
}