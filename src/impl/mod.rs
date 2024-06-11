
#[cfg(feature="std")]
pub mod std_impl {
    extern crate std;

    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use log::info;
    use crate::fifo::AtomicTimestampsRing;
    use crate::granular_buf::GranularBuf;
    use crate::tracing::{SharedTraceBufferTrait, TimestampImpl, TraceCollector, Tracer};

    pub static FAILED_LOCKS: AtomicUsize = AtomicUsize::new(0);
    pub struct ThreadTraceCollector(std::thread::JoinHandle<()>);

    impl<R: SharedTraceBufferTrait> TraceCollector<R> for ThreadTraceCollector {
        fn spawn(ringbuf: Arc<R>, is_finished: Arc<AtomicBool>) -> Self {
            let handle = std::thread::spawn(move || {
                let mut accum: u64 = 0;
                while !is_finished.load(Ordering::Relaxed) {
                    let bytes = ringbuf.try_pop::<50>();
                    if bytes.is_some() {
                        accum += 50;
                    }
                    else {
                        FAILED_LOCKS.fetch_add(1, Ordering::Relaxed);
                    }
                    // std::thread::sleep(Duration::from_nanos(1));
                    std::thread::yield_now();
                }
                info!("Finished! Total received bytes: {}", accum);
                info!("Failed try_pop calls: {}", FAILED_LOCKS.load(Ordering::Relaxed));
            });
            ThreadTraceCollector(handle)
        }

        fn wait_for_finish(self) {
            self.0.join().unwrap();
        }
    }

    pub struct SystemTimeTimestamp(std::time::SystemTime);

    impl TimestampImpl for SystemTimeTimestamp {
        fn now() -> Self {
            SystemTimeTimestamp(std::time::SystemTime::now())
        }

        fn elapsed_ns(&self) -> u64 {
            self.0.elapsed().unwrap().as_nanos() as u64
        }
    }

    pub type LockFreeTracer = Tracer<AtomicTimestampsRing, ThreadTraceCollector, SystemTimeTimestamp>;
}