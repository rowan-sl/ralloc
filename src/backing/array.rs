//! backing based on stack-allocated arrays

use super::Backing;

/// # Saftey
///
/// here we do a nice trick.
/// Backing is implemented **on a reference** so in this way, the reference prevents the referenced from moving,
/// deals with lifetime issues, and can be moved without messing things up all in one go
///
impl<const S: usize> Backing for &mut [u8; S] {
    fn get_mem(&mut self) -> &mut [u8] {
        *self
    }
}
