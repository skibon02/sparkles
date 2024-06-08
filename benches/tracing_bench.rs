#![feature(test)]
extern crate test;

use std::sync::{Arc, Mutex};
use test::{Bencher, black_box};
use tracer::tracing::{IS_FINISHED, Tracer};

fn perform_work() {
    let mut v = 0.0f64;
    for i in 0..1000 {
        v += (i as f64).sin();
    }
    black_box(v);
}

fn perform_work_with_tracing(tracer: Arc<Tracer>) {
    let mut v = 0.0f64;
    for i in 0..1000 {
        v += (i as f64).sin();
        tracer.event(1);
    }
    black_box(v);
}

fn perform_work_with_mutex(mutex: Arc<Mutex<i32>>) {
    let mut v = 0.0f64;
    for i in 0..1000 {
        v += (i as f64).sin();
        let data = mutex.lock().unwrap();
        *black_box(data) += v as i32;
    }
    black_box(v);
}

#[bench]
fn bench_arithmetic_operations(b: &mut Bencher) {
    b.iter(|| {
        perform_work();
    });
}

#[bench]
fn bench_arithmetic_operations_with_tracing(b: &mut Bencher) {
    let tracer = Arc::new(Tracer::new());

    b.iter(|| {
        perform_work_with_tracing(tracer.clone());
    });
    IS_FINISHED.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[bench]
fn bench_mutex_lock(b: &mut Bencher) {
    let mutex = Arc::new(Mutex::new(0));

    b.iter(|| {
        perform_work_with_mutex(mutex.clone());
    });
}