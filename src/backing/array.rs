//! backing based on stack-allocated arrays

use std::ptr::slice_from_raw_parts_mut;

use super::Backing;

/// # Saftey
///
/// here we do a nice trick.
/// Backing is implemented **on a reference** so in this way, the reference prevents the referenced from moving,
/// deals with lifetime issues, and can be moved without messing things up all in one go
///
unsafe impl<const S: usize> Backing for &mut [u8; S] {
    fn get_mem(&mut self) -> *mut [u8] {
        slice_from_raw_parts_mut((*self).as_mut_ptr(), (*self).len())
    }
}
