pub mod memmap;
pub mod array;
pub mod slice;

use crate::allocator::RAlloc;

/// # Saftey requirements
///
/// - for the function get_mem
///     - it MUST be safe to use the returned pointer AFTER Self has been moved
///
pub unsafe trait Backing {
    fn get_mem(&mut self) -> *mut [u8];
}

#[derive(Debug)]
pub struct BackedAllocator<'a, B: Backing + 'a>(/* drop order: ralloc is dropped before Backing */ RAlloc<'a>, B);

impl<'a, B: Backing + 'a> BackedAllocator<'a, B> {
    pub fn new(mut b: B) -> Self {
        // # Saftey
        // the contract of the unsafe trait Backing requires that the ptr
        // returned by this function stays valid AFTER the original value is moved.
        // (basically only ptrs, but it works anyway)
        // TODO: validate the lifetime stuff that happens here
        let alloc = RAlloc::new(unsafe { &mut*b.get_mem() });
        Self(alloc, b)
    }

    pub fn get_alloc(&mut self) -> &mut RAlloc<'a> {
        &mut self.0
    }
}
