#![no_std]
#![cfg_attr(all(target_arch = "xtensa", target_os = "espidf"), feature(asm_experimental_arch))]

extern crate alloc;

pub mod timestamp;
pub use timestamp::{Timestamp, TimestampProvider};

pub mod local_storage;
pub mod config;
pub mod consts;
pub mod protocol;