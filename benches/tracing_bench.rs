#![feature(test)]
extern crate test;

use std::sync::{Arc, Mutex};
use test::{Bencher, black_box};
use tracer::{granular_buf::GranularBuf, tracing::{Tracer, IS_FINISHED}};
use tracer::fifo::AtomicTimestampsRing;

pub type LockFreeTracer = Tracer<AtomicTimestampsRing>;

const N: usize = 10_000;

fn perform_work() {
    let mut v = 0.0f64;
    for i in 0..N {
        v += (i as f64 + 234.532).sqrt();
    }
    black_box(v);
}

fn perform_work_with_tracing(tracer: Arc<LockFreeTracer>) {
    let mut v = 0.0f64;
    for i in 0..N {
        v += (i as f64 + 234.532).sqrt();
        tracer.event(1);
    }
    black_box(v);
}


fn perform_work_with_tracing_2_threads(tracer: Arc<LockFreeTracer>) {
    let tracer2 = tracer.clone();
    let thread2 = std::thread::spawn(move || {
        let mut v = 0.0f64;
        for i in 0..N {
            v += (i as f64 + 234.532).sqrt();
            tracer2.event(2);
        }
        black_box(v);
    });
    let mut v = 0.0f64;
    for i in 0..N {
        v += (i as f64 + 234.532).sqrt();
        tracer.event(1);
    }
    black_box(v);
    thread2.join().unwrap();
}




fn perform_work_with_tracing_4_threads(tracer: Arc<LockFreeTracer>) {
    let tracer2 = tracer.clone();
    let tracer3 = tracer.clone();
    let tracer4 = tracer.clone();
    let thread2 = std::thread::spawn(move || {
        let mut v = 0.0f64;
        for i in 0..N {
            v += (i as f64 + 234.532).sqrt();
            tracer2.event(2);
        }
        black_box(v);
    });

    let thread3 = std::thread::spawn(move || {
        let mut v = 0.0f64;
        for i in 0..N {
            v += (i as f64 + 234.532).sqrt();
            tracer3.event(2);
        }
        black_box(v);
    });

    let thread4 = std::thread::spawn(move || {
        let mut v = 0.0f64;
        for i in 0..N {
            v += (i as f64 + 234.532).sqrt();
            tracer4.event(2);
        }
        black_box(v);
    });
    let mut v = 0.0f64;
    for i in 0..N {
        v += (i as f64 + 234.532).sqrt();
        tracer.event(1);
    }
    black_box(v);
    thread2.join().unwrap();
    thread3.join().unwrap();
    thread4.join().unwrap();
}

fn perform_only_tracing(tracer: Arc<LockFreeTracer>) {
    for _ in 0..N {
        tracer.event(1);
    }
}

fn perform_work_with_mutex(mutex: Arc<Mutex<i32>>) {
    let mut v = 0.0f64;
    for i in 0..N {
        v += (i as f64 + 234.532).sqrt();
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
fn bench_arithmetic_operations_with_tracing_2threads(b: &mut Bencher) {
    let tracer = Arc::new(Tracer::new());

    b.iter(|| {
        perform_work_with_tracing_2_threads(tracer.clone());
    });
    IS_FINISHED.store(true, std::sync::atomic::Ordering::Relaxed);
}


#[bench]
fn bench_arithmetic_operations_with_tracing_4threads(b: &mut Bencher) {
    let tracer = Arc::new(Tracer::new());

    b.iter(|| {
        perform_work_with_tracing_4_threads(tracer.clone());
    });
    IS_FINISHED.store(true, std::sync::atomic::Ordering::Relaxed);
}


#[bench]
fn bench_empty_tracing(b: &mut Bencher) {
    let tracer = Arc::new(Tracer::new());

    b.iter(|| {
        perform_only_tracing(tracer.clone());
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