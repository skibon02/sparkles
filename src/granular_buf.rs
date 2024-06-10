use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
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
    fn slice_buf_mut(&self, index: usize) -> &mut[u8] {
        &mut (unsafe { &mut *self.buf })[index*128..]
        //&mut (*self.mem)[index]
    }
}

static TRY_PUSH_CNT: AtomicUsize = AtomicUsize::new(0);
static TRY_POP_CNT: AtomicUsize = AtomicUsize::new(0);

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
                let buf = self.slice_buf_mut(slice_index);
                for (i, &v) in val.iter().enumerate() {
                    buf[buf_index as usize + i] = v;
                }

                // release lock and update counters 0x80 & !0x7f
                let new_counters = buf_index.wrapping_add(n);
                c.store(new_counters, Ordering::Relaxed);

                TRY_PUSH_CNT.fetch_add(1, Ordering::Relaxed);

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

                // We got access to range from [index * 128] to [(index+1) * 128]
                // pop values
                let mut res = [0; N as usize];
                let buf = self.slice_buf_mut(slice_index);
                for (i, v) in res.iter_mut().enumerate() {
                    *v = buf[buf_index as usize - 1 - i];
                }

                // release lock and update counter
                let new_counters = buf_index.wrapping_sub(N);
                c.store(new_counters, Ordering::Relaxed);

                TRY_POP_CNT.fetch_add(1, Ordering::Relaxed);

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

impl Drop for GranularBuf {
    fn drop(&mut self) {
        info!("try_push calls: {}", TRY_PUSH_CNT.load(Ordering::Relaxed));
        info!("try_pop calls: {}", TRY_POP_CNT.load(Ordering::Relaxed));
    }
}