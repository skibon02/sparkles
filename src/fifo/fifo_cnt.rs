use std::hint::spin_loop;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::fifo::RINGBUF_IND_MASK;

#[derive(Eq, PartialEq, Copy, Clone)]
pub struct LockFreeIndex(usize);

impl LockFreeIndex {
    #[inline(always)]
    pub(crate) const fn index(self) -> usize {
        self.0 >> 16
    }

    #[inline(always)]
    pub(crate) const fn in_process_count(self) -> u8 {
        self.0 as u8
    }

    #[inline(always)]
    pub(crate) const fn done_count(self) -> u8 {
        (self.0 >> 8) as u8
    }
}

impl From<usize> for LockFreeIndex {
    fn from(val: usize) -> LockFreeIndex {
        LockFreeIndex(val)
    }
}

impl From<LockFreeIndex> for usize {
    fn from(val: LockFreeIndex) -> usize {
        val.0
    }
}

pub fn counter_len(read_counters: LockIndex, write_counters: LockFreeIndex, cap: usize) -> usize {
    let read_index = read_counters.index();
    let write_index = write_counters.index();
    let len = if read_index <= write_index { write_index - read_index } else { write_index + cap - read_index };
    //len is from read_index to write_index, but we have to subtract read_in_process_count for a better approximation
    len
}


#[cfg_attr(target_arch = "x86_64", repr(align(128)))]
#[cfg_attr(not(target_arch = "x86_64"), repr(align(64)))]
pub struct LockFreeIndexStore {
    counters: AtomicUsize,
}

impl LockFreeIndexStore {
    pub const fn new() -> LockFreeIndexStore {
        LockFreeIndexStore { counters: AtomicUsize::new(0) }
    }
    #[inline(always)]
    pub fn load(&self, ordering: Ordering) -> LockFreeIndex {
        LockFreeIndex(self.counters.load(ordering))
    }

    /// Start operation, may fail
    ///
    /// There is a low probability of LockFreeIndex value being changed in process of writing,
    /// so we return it to reuse in the following increment_done call.
    #[inline(always)]
    pub fn increment_in_progress<F>(&self, error_condition: F, n: u8) -> Result<(LockFreeIndex, usize), ()>
        where F: Fn(usize, u8) -> bool {

        // Mark write as in progress
        let mut counters = self.load(Ordering::Acquire);
        loop {
            let in_progress_count = counters.in_process_count();
            if error_condition(counters.index(), in_progress_count) {
                return Err(());
            }

            // spin wait on MAXIMUM_IN_PROGRESS simultaneous in progress writes/reads
            if in_progress_count + n > super::MAX_IN_PROGRESS_BYTES_WRITE {
                spin_loop();
                counters = self.load(Ordering::Acquire);
                continue;
            }
            let index = counters.index().wrapping_add(in_progress_count as usize) & RINGBUF_IND_MASK;

            // recheck error condition, e.g. full/empty
            if error_condition(index, in_progress_count) {
                return Err(());
            }

            let new_counters = LockFreeIndex(counters.0.wrapping_add(n as usize));
            match self.counters.compare_exchange_weak(counters.0, new_counters.0, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => return Ok((new_counters, index)),
                Err(updated) => counters = LockFreeIndex(updated)
            };
        }
    }

    /// Finalize operation, may not fail
    #[inline(always)]
    pub fn increment_done(&self, mut counters: LockFreeIndex, n: u8) {
        loop {
            let in_process_count = counters.in_process_count();
            let new_counters = LockFreeIndex(if counters.done_count().wrapping_add(n) == in_process_count {
                // if the new done_count equals in_process_count count commit:
                // increment read_index and zero read_in_process_count and read_done_count
                (counters.index().wrapping_add(in_process_count as usize) & RINGBUF_IND_MASK) << 16
            } else {
                // otherwise we just increment read_done_count
                counters.0.wrapping_add((n as usize) << 8)
            });


            match self.counters.compare_exchange_weak(counters.0, new_counters.0, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => return,
                Err(updated) => counters = LockFreeIndex(updated)
            };
        }
    }
}

// Index counter with "locked" flag: no concurrency, designed for fast operations
#[derive(Eq, PartialEq, Copy, Clone)]
pub struct LockIndex(usize);

impl LockIndex {
    #[inline(always)]
    pub(crate) const fn index(self) -> usize {
        self.0 & 0xFF_FF
    }

    #[inline(always)]
    pub(crate) const fn is_locked(self) -> bool {
        self.0 & 0x10000 != 0
    }
}

impl From<usize> for LockIndex {
    fn from(val: usize) -> LockIndex {
        LockIndex(val)
    }
}

impl From<LockIndex> for usize {
    fn from(val: LockIndex) -> usize {
        val.0
    }
}

#[cfg_attr(target_arch = "x86_64", repr(align(128)))]
#[cfg_attr(not(target_arch = "x86_64"), repr(align(64)))]
pub struct LockIndexStore {
    counters: AtomicUsize,
}


impl LockIndexStore {
    pub const fn new() -> LockIndexStore {
        LockIndexStore { counters: AtomicUsize::new(0) }
    }
    #[inline(always)]
    pub fn load(&self, ordering: Ordering) -> LockIndex {
        LockIndex(self.counters.load(ordering))
    }

    // Start operation, may fail
    #[inline(always)]
    pub fn increment_start<F>(&self, error_condition: F) -> Result<(LockIndex, usize), ()>
        where F: Fn(usize, bool) -> bool {

        // try to lock the index
        let mut counters = self.load(Ordering::Acquire);
        loop {
            if counters.is_locked() {
                spin_loop();
                counters = self.load(Ordering::Acquire);
                continue;
            }
            let index = counters.index();
            if error_condition(index, counters.is_locked()) {
                return Err(());
            }
            let new_counters = LockIndex(counters.0 | 0x10000); // set locked flag
            match self.counters.compare_exchange_weak(counters.0, new_counters.0, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => return Ok((new_counters, index)),
                Err(updated) => counters = LockIndex(updated)
            };
        }
    }

    // Finalize operation, may not fail
    #[inline(always)]
    pub fn increment_done(&self, mut counters: LockIndex, n: u8) {
        loop {
            let new_counters = LockIndex(counters.index().wrapping_add(n as usize) & RINGBUF_IND_MASK);

            // must be processed immediately, because no other worker can unlock it
            match self.counters.compare_exchange_weak(counters.0, new_counters.0, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => return,
                Err(updated) => counters = LockIndex(updated)
            };
        }
    }
}