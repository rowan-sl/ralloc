use core::{
    alloc::{AllocError, Allocator, Layout},
    cell::RefCell,
    ptr::NonNull,
};

#[cfg(all(feature="alloc", not(feature="std")))]
use alloc::rc::Rc;

#[cfg(all(feature="std", not(feature="alloc")))]
use std::rc::Rc;

use crate::backing::{BackedAllocator, Backing};

/// A shared wrapper around the allocator.
///
/// not thread safe.
///
/// It is recommended to use multiple allocators per-thread instead of one shared allocator,
/// because of the overhead and potential to deadlock.
#[derive(Debug)]
pub struct RAllocShared<B: Backing>(Rc<RefCell<BackedAllocator<B>>>);

impl<B: Backing> RAllocShared<B> {
    pub fn new(alloc: BackedAllocator<B>) -> Self {
        Self(Rc::new(RefCell::new(alloc)))
    }
}

/// because rust cant figure out that cloning RAllocShared does not clone `B`
impl<B: Backing> Clone for RAllocShared<B> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// note: this is ultimately implementing on a ptr (Rc), so in that way it kinda folows the sudgestions
unsafe impl<B: Backing> Allocator for RAllocShared<B> {
    #[must_use]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // Saftey
        //
        // the returned value is valid for as long as required by Allocator (untill all handles are dropped),
        // because the underlying allocator is stored in Rc<RefCell<T>> which means that it will not be dropped untill all handles are dropped, and
        // the allocator itself has a unique ref to the memory backing, so that cannot be dropped first
        unsafe {
            self.0
                .borrow_mut()
                .get_alloc()
                .allocator_compatable_malloc(layout)
        }
    }

    #[forbid(unsafe_op_in_unsafe_fn)]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // Saftey
        // convered by the requirements of the Allocator::deallocate
        unsafe {
            self.0
                .borrow_mut()
                .get_alloc()
                .allocator_compatable_free(ptr, layout)
        };
    }
}
