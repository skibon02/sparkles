#![feature(generic_const_exprs)]
#![feature(slice_ptr_get)]
#![no_std]

extern crate alloc;

pub mod tracing;
pub mod fifo;
pub mod granular_buf;
pub mod r#impl;