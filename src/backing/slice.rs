//! backing based on slices. this is rather questionable,
//! as other impls could be replaced by this one, but i **do NOT** care
//!
//! it makes more sense anyway (for things like memmap), because
//! the allocator thing can own the source

use std::ptr::slice_from_raw_parts_mut;

use super::Backing;

/// # Saftey
///
/// here we do a nice trick.
/// Backing is implemented **on a reference** so in this way, the reference prevents the referenced from moving,
/// deals with lifetime issues, and can be moved without messing things up all in one go
///
unsafe impl Backing for &mut [u8] {
    fn get_mem(&mut self) -> *mut [u8] {
        slice_from_raw_parts_mut((*self).as_mut_ptr(), (*self).len())
    }
}
