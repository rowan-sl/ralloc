use core::{
    alloc::{AllocError, Layout},
    mem::size_of,
    ptr::{slice_from_raw_parts_mut, slice_from_raw_parts, NonNull},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Metadata {
    size: usize,
    used: bool,
}

impl Metadata {
    pub const fn new(size: usize, used: bool) -> Self {
        Self { size, used }
    }

    pub fn to_bytes(self) -> [u8; Metadata::size()] {
        let mut data = [0u8; size_of::<usize>() + size_of::<bool>()];
        data.as_mut_slice()[0..size_of::<usize>()].copy_from_slice(&self.size.to_le_bytes()[..]);
        data.as_mut_slice()[8] = self.used as u8;
        data
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() == 9);
        let mut size_data = [0u8; size_of::<usize>()];
        size_data
            .as_mut_slice()
            .clone_from_slice(&bytes[0..size_of::<usize>()]);
        Self {
            size: usize::from_le_bytes(size_data),
            used: match bytes[8] {
                0 => false,
                1 => true,
                v => panic!("cannot convert {v} to a bool"),
            },
        }
    }

    pub const fn size() -> usize {
        size_of::<usize>() + size_of::<bool>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocatorMetadata {
    initialized: bool,
}

impl AllocatorMetadata {
    /// note: `AllocatorMetadata::new(false)` **MUST** be equivilant to `AllocatorMetadata::from_bytes(&[0u8; AllocatorMetadata::size()])`
    pub const fn new(initialized: bool) -> Self {
        Self { initialized }
    }

    // needs to be CTFE compatable
    pub fn to_bytes(self) -> [u8; AllocatorMetadata::size()] {
        let mut data = [0u8; AllocatorMetadata::size()];
        data.as_mut_slice()[0] = self.initialized as u8;
        data
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() == AllocatorMetadata::size());
        let initialized_byte = bytes[0];
        assert!(initialized_byte == 0 || initialized_byte == 1);
        Self {
            initialized: initialized_byte == 1,
        }
    }

    pub const fn size() -> usize {
        size_of::<bool>()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RAlloc {
    mem: NonNull<[u8]>,
}

impl RAlloc {
    /// Returns `None` if
    /// - mem is too small
    /// - mem is not zeroed
    ///
    /// # Saftey
    /// - mem must be valid to use for as long as this struct (and all memory allocated by it) exists
    pub const unsafe fn new(mem: NonNull<[u8]>) -> Option<Self> {
        // note for people working on this: it is VERY important that `mem` is not written to, as that will cause CTFE errors in some cases

        // CTFE assertions go brrrr
        // if this is false, then it will evalueate to -1, which is invalid for usize and will error at compile time
        // this just makes shure that the zeroing check is accurate
        const _ASSERT: usize = (AllocatorMetadata::size() == 1) as usize - 1;
        // we do not have to write the AllocatorMetadata becuase it is valid in the state we want if loaded from zeroed mem
        if (Self { mem }).mem(0, 1)[0] != 0 {
            return None;
        }
        const MIN_SIZE: usize = AllocatorMetadata::size() + Metadata::size();
        if (Self { mem }).capacity() < MIN_SIZE {
            return None;
        }
        Some(Self { mem })
    }

    /// # Saftey
    /// - mem must have come (directly or indirectly) from another allocator that had into_raw() called on it
    pub unsafe fn from_raw(mem: NonNull<[u8]>) -> Self {
        Self { mem }
    }

    /// equvilant to copying the pointer before constructing the allocator, dropping the allocator, and then returning the copied ptr
    pub fn into_raw(self) -> NonNull<[u8]> {
        self.mem
    }

    pub fn init(&mut self) {
        let mut alloc_meta =
            AllocatorMetadata::from_bytes(&self.mem(0, AllocatorMetadata::size()));
        if !alloc_meta.initialized {
            alloc_meta.initialized = true;
            self.mem_mut(0, AllocatorMetadata::size()).copy_from_slice(&alloc_meta.to_bytes()[..]);
            let capac = self
                .capacity()
                .checked_sub(AllocatorMetadata::size() + Metadata::size());
            // saftey: length of mem is validated in Self::new()
            let capac = unsafe { capac.unwrap_unchecked() };
            self.write_meta_at(
                0,
                Metadata {
                    size: capac,
                    used: false,
                },
            );
        }
    }

    /// # Saftey
    ///
    /// - (annoyingly) the returned NonNull<[u8]> is only valid for the lifetime of Self
    ///
    #[must_use]
    #[forbid(unsafe_op_in_unsafe_fn)]
    pub unsafe fn allocator_compatable_malloc(
        &mut self,
        layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        let align = layout.align();
        let size = layout.size();

        let mut ptr = unsafe { self.malloc(size + align)? };
        // * NOTE: currently, when running this under MIRI with -Zmiri-symbolic-alignment-check, this will allways return usize::MAX.
        // *       nothing we can do about it, untill they add that functionality (it is intentional) but other than this its fine :shrug:
        // assert_ne!(ptr.align_offset(align), usize::MAX);
        // make shure that the returned offset will not put us beyond the extra size of the allocation
        // assert!(ptr.align_offset(align) < align);
        // * NOTE 2: this is fixed with the round_up_to fn
        // let _before = ptr as usize;
        ptr = ptr.map_addr(|addr| round_up_to(addr, align));
        // let _after = ptr as usize;
        // offset the ptr
        // Saftey:
        // bounds of the offset are checked with the last two asserts
        // ptr = unsafe { ptr.add(ptr.align_offset(align)) };
        // let _after = ptr as usize;
        let slice_ptr = slice_from_raw_parts_mut(ptr, size);
        let nonnull = NonNull::new(slice_ptr).unwrap();
        Ok(nonnull)
    }

    /// # Saftey
    ///
    /// - ptr must have come from this allocator instance
    /// - it must never have been freed before
    /// - Layout must be a valid layout describing it (not necessary in practice, but may change)
    ///
    pub unsafe fn allocator_compatable_free(&mut self, ptr: NonNull<u8>, _layout: Layout) {
        let ptr = ptr.as_ptr();
        self.free(ptr);
    }

    /// Allocate a new chunk of size `size` and return a pointer to the start of the allocation
    ///
    /// # Lifetime of returned values / Saftey
    ///
    /// - The returned value is valid untill either the instance it came from is dropped, or it is passed to RAlloc::free
    ///
    /// # Alignment
    ///
    /// currently, this function provides no guarentees about the align of the returned ptr.
    ///
    #[must_use]
    #[forbid(unsafe_op_in_unsafe_fn)]
    pub unsafe fn malloc(&mut self, size: usize) -> Result<*mut u8, AllocError> {
        // lazy initialization wooo
        self.init();
        // println!("Allocating size {size}");
        self.defrag();
        // sentinal value for OOM (when no valid chunk is found)
        assert!(size < usize::MAX);
        let mut best: (usize, Metadata) = (0, Metadata::new(usize::MAX, true));
        let mut offset = usize::MAX; //see docs for next_chunk
        while let Some(n_offset) = self.next_chunk(offset) {
            offset = n_offset;
            let meta = self.read_meta_at(n_offset);
            if (!meta.used) && (meta.size > size) && (meta.size < best.1.size) {
                best = (offset, meta);
            }
        }
        if best.1.size == usize::MAX {
            // println!("Error: OOM (wanted {size})");
            return Err(AllocError);
        }
        let extra = best.1.size - size;
        const MIN_SIZE_TO_FRAGMENT: usize = 256; // random number lol. DEFINITALLY should be a power of 2
        const REAL_MIN_SIZE_TO_FRAGMENT: usize = MIN_SIZE_TO_FRAGMENT + Metadata::size();
        if extra > REAL_MIN_SIZE_TO_FRAGMENT {
            self.split_chunk(best.0, size);
        }
        assert!(offset + Metadata::size() < isize::MAX as usize);
        // # Saftey
        // the chunk is valid, and it most definitally came from this allocator
        // the uper bound of size is checked
        Ok(unsafe { self.use_chunk(best.0) })
    }

    /// # Note
    ///
    /// It is acceptable to return a pointer to the MIDDLE of a chunk, (for things like alignment)
    ///
    /// # Panics
    ///
    /// - if ptr is outside of the bounds of self.mem
    /// - if the chunk the pointer points to was already free
    ///
    /// # Saftey
    ///
    /// - ptr MUST be a valid chunk, that was returned by the malloc() method of **this** allocator
    /// - the chunk pointed to by ptr MUST be in use, and must have not been passed to this function before.
    ///
    pub unsafe fn free(&mut self, ptr: *mut u8) {
        // note: we do *not* call init() here, because it is called in malloc() and it is UB anyway to call free before malloc
        let raw_offset = self.offset_by_ptr(ptr);
        let mut offset = usize::MAX; //see docs for next_chunk
        while let Some(n_offset) = self.next_chunk(offset) {
            offset = n_offset;
            let meta = self.read_meta_at(n_offset);
            // println!("offset {}, raw_offset {}, min: {}, max: {}", offset, raw_offset, offset + Metadata::size(), offset + Metadata::size() + meta.size);
            if (offset + Metadata::size() <= raw_offset)
                && (raw_offset < offset + Metadata::size() + meta.size)
            {
                // we found it!
                self.set_chunk_used(offset, false);
                return;
            }
        }
        panic!("No valid chunk found for ptr! (you did something you really shouldent have)")
    }

    fn defrag(&mut self) {
        let mut offset = usize::MAX; //see docs for next_chunk
        while let Some(n_offset) = self.next_chunk(offset) {
            offset = n_offset;
            let meta = self.read_meta_at(n_offset);
            if !meta.used {
                // not including Metadata::size() since this is implicitly taken care of later, when the new metadata takes its place
                let mut overall_size = meta.size;
                let mut inner_offset = offset;
                while let Some(n_inner_offset) = self.next_chunk(inner_offset) {
                    inner_offset = n_inner_offset;
                    let n_meta = self.read_meta_at(inner_offset);
                    if !n_meta.used {
                        overall_size += Metadata::size() + n_meta.size;
                    } else {
                        break;
                    }
                }
                self.write_meta_at(
                    offset,
                    Metadata {
                        size: overall_size,
                        used: false,
                    },
                );
            }
        }
    }

    /// **NOTE:** to use this for iterating over chunks (from chunk 0), pass usize::MAX
    ///
    /// get the next chunk. returns None if there is no next chunk
    ///
    /// may panic if the chunk at offset is invalid, or offset is invalid
    fn next_chunk(&self, offset: usize) -> Option<usize> {
        //TODO a less hacky way of doing this
        if offset == usize::MAX {
            return Some(0);
        }
        let meta = self.read_meta_at(offset);
        let next_idx = offset + Metadata::size() + meta.size;
        if (Self::base_offset() + next_idx) >= self.capacity() {
            None
        } else {
            Some(next_idx)
        }
    }

    /// Gets the offset of a chunk in self.memory from a pointer to that memory
    ///
    /// # Panics
    ///
    /// - if ptr is outside of the bounds of self.mem
    /// - if ptr is misaligned for the type of self.mem (u8) (should be impossible since u8 (as a byte) should have an align of 1)
    ///
    /// # Saftey
    ///
    /// - ptr MUST point to the memory of a valid allocated chunk
    /// - that chunk must be allocated BY THIS ALLOCATOR
    ///
    unsafe fn offset_by_ptr(&self, ptr: *const u8) -> usize {
        offset_from(self.mem, ptr)
    }

    /// "uses" a chunk, setting it as taken and returning a pointer to its memory
    ///
    /// # Saftey
    ///
    /// - a valid chunk (Metadata, [u8]) must exist in self.memory, begining at offset, and that chunk **must not be in use!**
    /// - offset + START_META_SIZE must fit into isize
    ///
    unsafe fn use_chunk(&mut self, offset: usize) -> *mut u8 {
        self.set_chunk_used(offset, true);
        self.mem
            .as_mut_ptr()
            .add(Self::base_offset() + offset + Metadata::size())
    }

    fn set_chunk_used(&mut self, offset: usize, used: bool) {
        let Metadata {
            size,
            used: prev_used,
        } = self.read_meta_at(offset);
        if used {
            assert!(!prev_used, "cannot allocate an already allocated chunk!");
        } else {
            assert!(prev_used, "cannot free an already freed chunk!");
        }
        self.write_meta_at(offset, Metadata { size, used });
    }

    fn split_chunk(&mut self, offset: usize, new_size: usize) {
        let Metadata { size, used } = self.read_meta_at(offset);
        assert!(!used, "cannot split an in-use chunk!");
        // the `+ 1` is not here because it needs to be, its here because otherwise you could get zero sized chunks (useless)
        assert!(
            size >= new_size + Metadata::size() + 1,
            "New size is too small to fit another section!"
        );
        let left_size = new_size;
        let right_size = size - new_size + Metadata::size();
        self.write_meta_at(
            offset,
            Metadata {
                size: left_size,
                used: false,
            },
        );
        self.write_meta_at(
            offset + Metadata::size() + left_size,
            Metadata {
                size: right_size,
                used: false,
            },
        );
    }

    fn read_meta_at(&self, mut offset: usize) -> Metadata {
        offset += Self::base_offset();
        // Metadata::from_bytes(&self.mem()[offset..offset + Metadata::size()])
        Metadata::from_bytes(self.mem(offset, Metadata::size()))
    }

    fn write_meta_at(&mut self, mut offset: usize, meta: Metadata) {
        offset += Self::base_offset();
        // self.mem_mut()[offset..offset + Metadata::size()].copy_from_slice(&meta.to_bytes()[..])
        self.mem_mut(offset, Metadata::size()).copy_from_slice(meta.to_bytes().as_slice());
    }

    const fn capacity(&self) -> usize {
        self.mem.len()
    }

    const fn base_offset() -> usize {
        AllocatorMetadata::size()
    }

    const fn mem(&self, offset: usize, len: usize) -> &[u8] {
        unsafe { &*slice_from_raw_parts(self.mem.as_ptr().as_mut_ptr().offset(offset as isize), len) }
    }

    fn mem_mut(&mut self, offset: usize, len: usize) -> &mut [u8] {
        unsafe { &mut *slice_from_raw_parts_mut(self.mem.as_ptr().as_mut_ptr().offset(offset as isize), len) }
    }
}

/// Returns the offset of `ptr` into `slice`.
/// Panics if `ptr` points to a location outside the slice or is misaligned.
fn offset_from<T>(slice: NonNull<[T]>, ptr: *const T) -> usize {
    assert!(
        ptr as usize % core::mem::align_of::<T>() == 0,
        "bad alignment"
    );
    assert!((slice.as_ptr().cast::<T>()..unsafe { slice.as_ptr().cast::<T>().offset(slice.len() as isize) }).contains(&(ptr as _)), "index oob");
    unsafe { ptr.offset_from(slice.as_ptr().cast::<T>()) as usize }
}

#[inline]
pub(crate) fn round_up_to(n: usize, divisor: usize) -> usize {
    debug_assert!(divisor.is_power_of_two());
    (n + divisor - 1) & !(divisor - 1)
}
