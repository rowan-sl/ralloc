//! backing based on slices. this is rather questionable,
//! as other impls could be replaced by this one, but i **do NOT** care
//!
//! it makes more sense anyway (for things like memmap), because
//! the allocator thing can own the source

use super::Backing;

impl Backing for &mut [u8] {
    fn get_mem(&mut self) -> &mut [u8] {
        *self
    }
}
