#![no_std]
extern crate alloc;

pub mod timestamp;
pub use timestamp::{Timestamp, TimestampProvider};

pub mod local_storage;
pub mod headers;
pub mod config;

pub const ENCODER_VER: u32 = 0;