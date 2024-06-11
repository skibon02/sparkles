#![feature(generic_const_exprs)]
#![no_std]

extern crate alloc;

pub mod tracing;
pub mod fifo;
pub mod granular_buf;
pub mod r#impl;