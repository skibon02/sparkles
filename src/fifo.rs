use std::fmt;
use std::ops::Range;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use crate::fifo::fifo_cnt::{counter_len, LockFreeIndexStore, LockIndexStore};

mod fifo_cnt;

/// Must be 2^n - 1
///
/// It is a mask for the index for the ring buffer
///
/// Size is mask + 1
const RINGBUF_IND_MASK: usize = 1023;
const RINGBUF_IND_CLEANUP_SIZE: usize = 500;
const MAX_IN_PROGRESS_BYTES_WRITE: u8 = 6;

pub struct AtomicTimestampsRing {
    buf: *mut [u8],
    write_ind : LockFreeIndexStore,
    read_ind: LockIndexStore,
}

unsafe impl Send for AtomicTimestampsRing {}
unsafe impl Sync for AtomicTimestampsRing {}

impl AtomicTimestampsRing {

    pub fn new() -> Self {

        let mut vec = Vec::with_capacity(RINGBUF_IND_MASK + 1);
        unsafe { vec.set_len(RINGBUF_IND_MASK + 1); }
        let buf = Box::into_raw(vec.into_boxed_slice());
        Self {
            buf,
            read_ind: LockIndexStore::new(),
            write_ind: LockFreeIndexStore::new(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        let read_counters = self.read_ind.load(Ordering::SeqCst);
        let write_counters = self.write_ind.load(Ordering::SeqCst);
        counter_len(read_counters, write_counters, self.capacity())
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        unsafe { (*self.buf).len() }
    }

    #[inline]
    pub fn remaining_cap(&self) -> usize {
        let read_counters = self.read_ind.load(Ordering::SeqCst);
        let write_counters = self.write_ind.load(Ordering::SeqCst);
        let cap = self.capacity();
        let read_index = read_counters.index();
        let write_index = write_counters.index();
        let len = if read_index <= write_index { write_index - read_index } else { write_index + cap - read_index };
        //len is from read_index to write_index, but we have to substract write_in_process_count for a better remaining capacity approximation
        cap - 1 - len - write_counters.in_process_count() as usize
    }

    /// Returns a *mut T pointer to an indexed cell
    #[inline(always)]
    unsafe fn cell(&self, index: usize) -> *mut u8 {
        (*self.buf).get_unchecked_mut(index)
        //&mut (*self.mem)[index]
    }
    //
    // /// write two bytes into fifo
    // fn put_two(&self, v1: u8, v2: u8) {
    //     unimplemented!()
    // }

    pub fn try_push(&self, v: u8) -> Option<()> {
        self.try_push_with(1, |_, _| v)
    }

    pub fn try_push_with<W>(&self, n: u8, mut writer: W) -> Option<()>
        where W: FnMut(usize, usize) -> u8 {
        /// Error condition is when the next index is the read index
        let error_condition = |to_write_index: usize, _: u8| {
            let read_ind = self.read_ind.load(Ordering::SeqCst).index();
            !can_push(read_ind, to_write_index, n, RINGBUF_IND_MASK)

            // to_write_index.wrapping_add(1) & RINGBUF_IND_MASK == self.read_ind.load(Ordering::SeqCst).index()
        };

        if let Ok((write_counters, to_write_index)) = self.write_ind.increment_in_progress(error_condition, n) {
            // n bytes are available for writing starting from to_write_index

            // write mem
            unsafe {
                *self.cell(to_write_index) = writer(to_write_index, (to_write_index + n as usize) & RINGBUF_IND_MASK);
            };

            // Mark write as done
            self.write_ind.increment_done(write_counters, n);
            Some(())
        } else {
            None
        }
    }

    pub fn try_pop<const N: u8>(&self) -> Option<[u8; N as usize]> {
        let n = N;
        let error_condition = |to_read_index: usize, _: bool| {
            let write_index = self.write_ind.load(Ordering::SeqCst).index();
            !can_pop(to_read_index, write_index, n, RINGBUF_IND_MASK)
            // to_read_index == self.write_ind.load(Ordering::SeqCst).index()
        };

        if let Ok((read_counters, to_read_index)) = self.read_ind.increment_start(error_condition) {
            let mut popped = [0; N as usize];
            // read mem
            unsafe {
                for i in 0..n as usize {
                    popped[i] = *self.cell((to_read_index + i) & RINGBUF_IND_MASK);
                }
            }
            self.read_ind.increment_done(read_counters, n);
            Some(popped)
        } else {
            None
        }
    }
}

#[inline(always)]
fn can_pop(r: usize, w: usize, n: u8, index_mask: usize) -> bool {
    (w + index_mask + 1 - r) & index_mask >= n as usize
}

#[inline(always)]
fn can_push(r: usize, w: usize, n: u8, index_mask: usize) -> bool {
    (index_mask + r - w) & index_mask >= n as usize
}

impl Drop for AtomicTimestampsRing {
    fn drop(&mut self) {
        unsafe {
            let _ = Box::from_raw(self.buf);
        }
    }
}

impl fmt::Debug for AtomicTimestampsRing {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            let cap = self.capacity();
            let read_counters = self.read_ind.load(Ordering::Relaxed);
            let write_counters = self.write_ind.load(Ordering::Relaxed);
            write!(f, "AtomicRingBuffer cap: {} len: {} write_index: {}, write_in_process_count: {}, write_done_count: {}, read_index: {}, read_is_locked: {}", cap, self.len(),
                   write_counters.index(), write_counters.in_process_count(), write_counters.done_count(),
                   read_counters.index(), read_counters.is_locked())
        } else {
            write!(f, "AtomicRingBuffer cap: {} len: {}", self.capacity(), self.len())
        }
    }
}

pub mod tests {
    use super::can_pop;
    use super::can_push;
    #[test]
    fn can_pop_test_1n() {
        let index_mask = 3;
        assert_eq!(can_pop(0, 0, 1, index_mask), false); // from the truth table
        assert_eq!(can_pop(0, 1, 1, index_mask), true);
        assert_eq!(can_pop(0, 2, 1, index_mask), true);
        assert_eq!(can_pop(0, 3, 1, index_mask), true);
        assert_eq!(can_pop(1, 0, 1, index_mask), true);
        assert_eq!(can_pop(1, 1, 1, index_mask), false);
        assert_eq!(can_pop(1, 2, 1, index_mask), true);
        assert_eq!(can_pop(1, 3, 1, index_mask), true);
        assert_eq!(can_pop(2, 0, 1, index_mask), true);
        assert_eq!(can_pop(2, 1, 1, index_mask), true);
        assert_eq!(can_pop(2, 2, 1, index_mask), false);
        assert_eq!(can_pop(2, 3, 1, index_mask), true);
        assert_eq!(can_pop(3, 0, 1, index_mask), true);
        assert_eq!(can_pop(3, 1, 1, index_mask), true);
        assert_eq!(can_pop(3, 2, 1, index_mask), true);
        assert_eq!(can_pop(3, 3, 1, index_mask), false);
    }

    #[test]
    fn can_pop_test_2n() {
        let index_mask = 3;
        assert_eq!(can_pop(0, 0, 2, index_mask), false);
        assert_eq!(can_pop(0, 1, 2, index_mask), false);
        assert_eq!(can_pop(0, 2, 2, index_mask), true);
        assert_eq!(can_pop(0, 3, 2, index_mask), true);
        assert_eq!(can_pop(1, 0, 2, index_mask), true);
        assert_eq!(can_pop(1, 1, 2, index_mask), false);
        assert_eq!(can_pop(1, 2, 2, index_mask), false);
        assert_eq!(can_pop(1, 3, 2, index_mask), true);
        assert_eq!(can_pop(2, 0, 2, index_mask), true);
        assert_eq!(can_pop(2, 1, 2, index_mask), true);
        assert_eq!(can_pop(2, 2, 2, index_mask), false);
        assert_eq!(can_pop(2, 3, 2, index_mask), false);
        assert_eq!(can_pop(3, 0, 2, index_mask), false);
        assert_eq!(can_pop(3, 1, 2, index_mask), true);
        assert_eq!(can_pop(3, 2, 2, index_mask), true);
        assert_eq!(can_pop(3, 3, 2, index_mask), false);
    }

    #[test]
    fn can_pop_test_3n() {
        let index_mask = 3;
        assert_eq!(can_pop(0, 0, 3, index_mask), false);
        assert_eq!(can_pop(0, 1, 3, index_mask), false);
        assert_eq!(can_pop(0, 2, 3, index_mask), false);
        assert_eq!(can_pop(0, 3, 3, index_mask), true);
        assert_eq!(can_pop(1, 0, 3, index_mask), true);
        assert_eq!(can_pop(1, 1, 3, index_mask), false);
        assert_eq!(can_pop(1, 2, 3, index_mask), false);
        assert_eq!(can_pop(1, 3, 3, index_mask), false);
        assert_eq!(can_pop(2, 0, 3, index_mask), false);
        assert_eq!(can_pop(2, 1, 3, index_mask), true);
        assert_eq!(can_pop(2, 2, 3, index_mask), false);
        assert_eq!(can_pop(2, 3, 3, index_mask), false);
        assert_eq!(can_pop(3, 0, 3, index_mask), false);
        assert_eq!(can_pop(3, 1, 3, index_mask), false);
        assert_eq!(can_pop(3, 2, 3, index_mask), true);
        assert_eq!(can_pop(3, 3, 3, index_mask), false);
    }

    #[test]
    fn can_push_test_1n() {
        let index_mask = 3;
        assert_eq!(can_push(0, 0, 1, index_mask), true);
        assert_eq!(can_push(0, 1, 1, index_mask), true);
        assert_eq!(can_push(0, 2, 1, index_mask), true);
        assert_eq!(can_push(0, 3, 1, index_mask), false);
        assert_eq!(can_push(1, 0, 1, index_mask), false);
        assert_eq!(can_push(1, 1, 1, index_mask), true);
        assert_eq!(can_push(1, 2, 1, index_mask), true);
        assert_eq!(can_push(1, 3, 1, index_mask), true);
        assert_eq!(can_push(2, 0, 1, index_mask), true);
        assert_eq!(can_push(2, 1, 1, index_mask), false);
        assert_eq!(can_push(2, 2, 1, index_mask), true);
        assert_eq!(can_push(2, 3, 1, index_mask), true);
        assert_eq!(can_push(3, 0, 1, index_mask), true);
        assert_eq!(can_push(3, 1, 1, index_mask), true);
        assert_eq!(can_push(3, 2, 1, index_mask), false);
        assert_eq!(can_push(3, 3, 1, index_mask), true);
    }

    #[test]
    fn can_push_test_2n() {
        let index_mask = 3;
        assert_eq!(can_push(0, 0, 2, index_mask), true);
        assert_eq!(can_push(0, 1, 2, index_mask), true);
        assert_eq!(can_push(0, 2, 2, index_mask), false);
        assert_eq!(can_push(0, 3, 2, index_mask), false);
        assert_eq!(can_push(1, 0, 2, index_mask), false);
        assert_eq!(can_push(1, 1, 2, index_mask), true);
        assert_eq!(can_push(1, 2, 2, index_mask), true);
        assert_eq!(can_push(2, 0, 2, index_mask), false);
        assert_eq!(can_push(2, 1, 2, index_mask), false);
        assert_eq!(can_push(2, 2, 2, index_mask), true);
        assert_eq!(can_push(2, 3, 2, index_mask), true);
        assert_eq!(can_push(3, 0, 2, index_mask), true);
        assert_eq!(can_push(3, 1, 2, index_mask), false);
        assert_eq!(can_push(3, 2, 2, index_mask), false);
        assert_eq!(can_push(3, 3, 2, index_mask), true);
    }


    #[test]
    fn push_pop() {
        let ring = super::AtomicTimestampsRing::new();
        assert_eq!(ring.try_pop(), None);
        assert_eq!(ring.try_push(1), Some(()));
        assert_eq!(ring.try_push(2), Some(()));
        assert_eq!(ring.try_push(3), Some(()));
        assert_eq!(ring.try_push(4), Some(()));
        assert_eq!(ring.try_pop(), None);
        assert_eq!(ring.try_push(5), Some(()));
        assert_eq!(ring.try_pop(), Some([1,2,3,4,5]));
        assert_eq!(ring.try_pop(), None);

    }
}