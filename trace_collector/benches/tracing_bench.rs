#![feature(test)]
extern crate test;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use test::{Bencher, black_box};
use tracing::{Dispatch, dispatcher, instrument};
use tracing_timing::Histogram;

const N: usize = 10_000;

fn perform_work() {
    let mut v = 0.0f64;
    for i in 0..N {
        v += (i as f64 + 234.532).sqrt();
    }
    black_box(v);
}

fn perform_work_with_tracing() {
    let mut v = 0.0f64;
    for i in 0..N {
        v += (i as f64 + 234.532).sqrt();
        tracer::event(23);
    }
    black_box(v);
}


fn perform_work_with_tracing_2_threads() {
    let thread2 = std::thread::spawn(move || {
        let mut v = 0.0f64;
        for i in 0..N {
            v += (i as f64 + 234.532).sqrt();
            tracer::event(2);
        }
        black_box(v);
    });
    let mut v = 0.0f64;
    for i in 0..N {
        v += (i as f64 + 234.532).sqrt();
        tracer::event(1);
    }
    black_box(v);
    thread2.join().unwrap();
}




fn perform_work_with_tracing_4_threads() {
    let thread2 = std::thread::spawn(move || {
        let mut v = 0.0f64;
        for i in 0..N {
            v += (i as f64 + 234.532).sqrt();
            tracer::event(2);
        }
        black_box(v);
    });

    let thread3 = std::thread::spawn(move || {
        let mut v = 0.0f64;
        for i in 0..N {
            v += (i as f64 + 234.532).sqrt();
            tracer::event(3);
        }
        black_box(v);
    });

    let thread4 = std::thread::spawn(move || {
        let mut v = 0.0f64;
        for i in 0..N {
            v += (i as f64 + 234.532).sqrt();
            tracer::event(4);
        }
        black_box(v);
    });
    let mut v = 0.0f64;
    for i in 0..N {
        v += (i as f64 + 234.532).sqrt();
        tracer::event(5);
    }
    black_box(v);
    thread2.join().unwrap();
    thread3.join().unwrap();
    thread4.join().unwrap();
}

fn perform_only_tracing() {
    for _ in 0..N {
        tracer::event(1);
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

fn calc_sqrt(val: f64) -> f64 {
    val.sqrt()
}


#[bench]
fn bench_tracing_library(b: &mut Bencher) {
    dispatcher::set_global_default(
        Dispatch::new(tracing_timing::Builder::default().build(|| {
            Histogram::new_with_max(1_000_000, 2).unwrap()
        })),
    ).unwrap();
    b.iter(|| {
        let mut v = 0.0f64;
        for i in 0..N {
            v += calc_sqrt(i as f64 + 234.532);
            tracing::info!("srqt calc");
        }
        black_box(v);
    });
}

#[bench]
fn bench_arithmetic_operations(b: &mut Bencher) {
    b.iter(|| {
        perform_work();
    });
    tracer::flush();
}

#[bench]
fn bench_arithmetic_operations_with_tracing(b: &mut Bencher) {
    b.iter(|| {
        perform_work_with_tracing();
    });
    tracer::flush();
}


#[bench]
fn bench_arithmetic_operations_with_tracing_2threads(b: &mut Bencher) {
    b.iter(|| {
        perform_work_with_tracing_2_threads();
    });
    tracer::flush();
}


#[bench]
fn bench_arithmetic_operations_with_tracing_4threads(b: &mut Bencher) {
    b.iter(|| {
        perform_work_with_tracing_4_threads();
    });
    tracer::flush();
}


#[bench]
fn bench_empty_tracing(b: &mut Bencher) {

    b.iter(|| {
        perform_only_tracing();
    });
    tracer::flush();
}

#[bench]
fn bench_mutex_lock(b: &mut Bencher) {
    let mutex = Arc::new(Mutex::new(0));

    b.iter(|| {
        perform_work_with_mutex(mutex.clone());
    });
}


#[bench]
fn bench_tls(b: &mut Bencher) {
    thread_local! {
        static CNT: RefCell<usize> = RefCell::new(0);
    }

    b.iter(|| {
        let mut v = 0.0f64;
        for i in 0..N {
            v += (i as f64 + 234.532).sqrt();
            CNT.with_borrow_mut(|v| *v+=1);
        }
        black_box(v);
    });

    log::info!("TLS counter: {}", CNT.with(|v| *v.borrow()));
}