pub mod array;
#[cfg(feature="std")]
pub mod memmap;
pub mod slice;

use core::ptr::NonNull;

use crate::allocator::RAlloc;

pub trait Backing {
    fn get_mem(&mut self) -> &mut [u8];
}

#[derive(Debug)]
pub struct BackedAllocator<B: Backing>(
    B,
    RAlloc, /* this allows us to hand out mutable references to an allocator,
            allowing better guarentees. although this may be invalid at any time, it is still fine because the pointer the
            allocator contains will never be dereferenced and so no rules are broken. this should be replaced every time get_alloc is called. */
);

impl<B: Backing> BackedAllocator<B> {
    pub fn new(mut b: B) -> Option<Self> {
        b.get_mem().fill(0);
        let alloc = unsafe { RAlloc::new(NonNull::from(b.get_mem()))? };
        Some(Self(b, alloc))
    }

    pub fn get_alloc(&mut self) -> &mut RAlloc {
        // fine since Ralloc::into_raw() is equivelant to just dropping self
        self.1 = unsafe { RAlloc::from_raw(NonNull::from(self.0.get_mem())) };
        &mut self.1
    }
}
