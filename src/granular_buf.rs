use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ptr;
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use log::info;

use crate::tracing::SharedTraceBufferTrait;

// SIZE is fixed at 127 * 4 splits


const SPLITS: usize = 4;


pub struct GranularBuf {
    buf: *mut [u8],
    counters: [AtomicU8; 4],
}

impl GranularBuf {
    #[inline(always)]
    fn slice_buf_raw(&self, index: usize) -> *mut u8 {
        unsafe { self.buf.as_mut_ptr().add(index << 7) }
    }
}

unsafe impl Send for GranularBuf {}
unsafe impl Sync for GranularBuf {}

impl SharedTraceBufferTrait for GranularBuf {
    fn try_push(&self, val: &[u8]) -> Option<()> {
        let n = val.len() as u8;

        // 4 attempts to write
        for (slice_index, c) in self.counters.iter().enumerate() {
            if let Ok(v) = c.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |c| {
                // check if lock bit is still 0
                if c & 0x80 == 0 && c & 0x7F <= 127 - n {
                    Some((c as u32 | 0x80) as u8)
                } else {
                    None
                }
            }) {
                let buf_index = v & 0x7F;

                // We got access to range from [index * 128] to [(index+1) * 128]
                // write values
                unsafe { ptr::copy_nonoverlapping(val.as_ptr(), self.slice_buf_raw(slice_index).add(buf_index as usize), n as usize); }

                // release lock and update counters 0x80 & !0x7f
                let new_counters = buf_index.wrapping_add(n);
                c.store(new_counters, Ordering::Relaxed);


                return Some(())
            }
        }
        None
    }

    fn try_pop<const N: u8>(&self) -> Option<[u8; N as usize]> {
        // 4 attempts to write
        for (slice_index, c) in self.counters.iter().enumerate() {
            if let Ok(v) = c.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |c| {
                // check if lock bit is still 0
                if c & 0x80 == 0 && c & 0x7F >= N {
                    Some((c as u32 | 0x80) as u8)
                } else {
                    None
                }
            }) {
                let buf_index = v & 0x7F;

                let mut res = [0; N as usize];
                // We got access to range from [index * 128] to [(index+1) * 128]
                // pop values
                unsafe { ptr::copy_nonoverlapping(self.slice_buf_raw(slice_index).add((buf_index - N) as usize), res.as_mut_ptr(), N as usize); }

                // release lock and update counter
                let new_counters = buf_index.wrapping_sub(N);
                c.store(new_counters, Ordering::Relaxed);

                return Some(res)
            }
        }
        None
    }

    fn new() -> Self {
        let mut vec = Vec::with_capacity(128*4);
        unsafe { vec.set_len(128*4); }
        let buf = Box::into_raw(vec.into_boxed_slice());

        const ARRAY_REPEAT_VALUE: AtomicU8 = AtomicU8::new(0);
        Self {
            buf,
            counters: [ARRAY_REPEAT_VALUE; 4]
        }
    }
}

pub mod test {
    #[test]
    pub fn buf_operations() {
        use crate::granular_buf::GranularBuf;
        use crate::tracing::SharedTraceBufferTrait;

        let buf = GranularBuf::new();

        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let data2 = [9, 10, 11, 12, 13, 14, 15, 16];

        buf.try_push(&data).unwrap();
        buf.try_push(&data2).unwrap();

        let res = buf.try_pop::<8>().unwrap();
        assert_eq!(res, data2);

        let res = buf.try_pop::<8>().unwrap();
        assert_eq!(res, data);
    }
}