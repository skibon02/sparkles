#![no_std]
#![cfg_attr(all(target_arch = "xtensa", target_os = "espidf"), feature(asm_experimental_arch))]

extern crate alloc;

pub mod timestamp;
pub use timestamp::{Timestamp, TimestampProvider};

pub mod local_storage;
pub mod config;
pub mod consts;
pub mod protocol;

/// Representation of a static string name used in events
/// You should use `sparkles::static_name!("event name")` macro to create it
#[derive(Copy, Clone)]
pub struct StaticNameRepr {
    hash: u32,
    string: &'static str,
}

impl StaticNameRepr {
    /// You should use `sparkles::static_name!("event name")` macro to create it
    pub unsafe fn from_str_and_hash(string: &'static str, hash: u32) -> Self {
        Self {
            string,
            hash,
        }
    }
    /// Value representing an empty name.
    pub fn empty() -> Self {
        StaticNameRepr {
            hash: 0,
            string: "",
        }
    }
    pub fn hash(&self) -> u32 {
        self.hash
    }
    pub fn string(&self) -> &'static str {
        self.string
    }
}