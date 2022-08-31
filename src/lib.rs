#![feature(allocator_api)]
#![feature(const_mut_refs)]
#![feature(strict_provenance)]
#![feature(slice_ptr_get)]
#![cfg_attr(not(feature="std"), no_std)]

#[cfg(all(not(feature="std"), feature="alloc"))]
extern crate alloc;

pub mod allocator;
pub mod backing;
pub mod wrappers;
