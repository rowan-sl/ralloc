use core::{mem::size_of, alloc::{Allocator, AllocError}, ptr::{NonNull, slice_from_raw_parts_mut}};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Metadata {
    size: usize,
    used: bool,
}

impl Metadata {
    pub fn new(size: usize, used: bool) -> Self {
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
        size_data.as_mut_slice().clone_from_slice(&bytes[0..size_of::<usize>()]);
        Self {
            size: usize::from_le_bytes(size_data),
            used: match bytes[8] {
                0 => false,
                1 => true,
                v => panic!("cannot convert {v} to a bool")
            },
        }
    }

    pub const fn size() -> usize {
        size_of::<usize>() + size_of::<bool>()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RAlloc<'a> {
    mem: &'a mut [u8],
}

impl<'a> RAlloc<'a> {
    pub fn new(mem: &'a mut [u8]) -> Self {
        let mut s = Self {
            mem,
        };
        let capac = s.mem.len().checked_sub(Metadata::size()).expect("size of memory must be large enough to fit at least one chunk!");
        s.write_meta_at(0, Metadata { size: capac, used: false });
        s
    }

    //* can const do ANYTHING???! */
    /// RAlloc::new(), but const.
    ///
    /// # Saftey
    ///
    /// - mem must be valid, and all that great stuff
    /// - the ptr to mem must be valid for the lifetime 'a (on Self)
    /// - mem must be large enough to fit at the minimum chunk size (Metadata::size)
    ///
    pub const unsafe fn new_const(mem: *mut [u8]) -> Self {
        let s = Self {
            mem: &mut*mem,
        };
        // write the metadata (i hate CTFE)
        // for metadata (size)
        let size = (*mem).len();
        let size_bytes = size.to_le_bytes();
        if size_bytes.len() == 2 {
            s.mem[0] = size_bytes[0];
            s.mem[1] = size_bytes[1];
        } else if size_bytes.len() == 4 {
            s.mem[0] = size_bytes[0];
            s.mem[1] = size_bytes[1];
            s.mem[2] = size_bytes[2];
            s.mem[3] = size_bytes[3];
        } else if size_bytes.len() == 8 {
            s.mem[0] = size_bytes[0];
            s.mem[1] = size_bytes[1];
            s.mem[2] = size_bytes[2];
            s.mem[3] = size_bytes[3];
            s.mem[4] = size_bytes[4];
            s.mem[5] = size_bytes[5];
            s.mem[6] = size_bytes[6];
            s.mem[7] = size_bytes[7];
        } else {
            panic!("Usize is not size 2, 4, or 8 (what are you running this on????)")
        }
        // for unused flag
        s.mem[8] = 0;
        s
    }

    /// Allocate a new chunk of size `size` and return a pointer to the start of the allocation
    ///
    /// # Alignment
    ///
    /// currently, this function provides no guarentees about the align of the returned ptr.
    ///
    #[must_use]
    pub fn malloc(&mut self, size: usize) -> Result<*mut u8, AllocError> {
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
            return Err(AllocError)
        }
        let extra = best.1.size - size;
        const MIN_SIZE_TO_FRAGMENT: usize = 256;// random number lol. DEFINITALLY should be a power of 2
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
        let raw_offset = self.offset_by_ptr(ptr);
        let mut offset = usize::MAX; //see docs for next_chunk
        while let Some(n_offset) = self.next_chunk(offset) {
            offset = n_offset;
            let meta = self.read_meta_at(n_offset);
            // println!("offset {}, raw_offset {}, min: {}, max: {}", offset, raw_offset, offset + Metadata::size(), offset + Metadata::size() + meta.size);
            if (offset + Metadata::size() <= raw_offset) && (raw_offset < offset + Metadata::size() + meta.size) {
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
                self.write_meta_at(offset, Metadata { size: overall_size, used: false });
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
            return Some(0)
        }
        let meta = self.read_meta_at(offset);
        let next_idx =  offset + Metadata::size() + meta.size;
        if next_idx >= self.capacity() {
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
        offset_from(&self.mem, ptr)
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
        self.mem.as_mut_ptr().add(offset + Metadata::size())
    }

    fn set_chunk_used(&mut self, offset: usize, used: bool) {
        let Metadata { size, used: prev_used } = self.read_meta_at(offset);
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
        assert!(size >= new_size + Metadata::size() + 1, "New size is too small to fit another section!");
        let left_size = new_size;
        let right_size = size - new_size + Metadata::size();
        self.write_meta_at(offset, Metadata { size: left_size, used: false });
        self.write_meta_at(offset + Metadata::size() + left_size, Metadata { size: right_size, used: false });
    }

    fn read_meta_at(&self, offset: usize) -> Metadata {
        Metadata::from_bytes(&self.mem[offset..offset + Metadata::size()])
    }

    fn write_meta_at(&mut self, offset: usize, meta: Metadata) {
        self.mem[offset..offset + Metadata::size()].copy_from_slice(&meta.to_bytes()[..])
    }

    fn capacity(&self) -> usize {
        self.mem.len()
    }
}


/// Returns the offset of `ptr` into `slice`.
/// Panics if `ptr` points to a location outside the slice or is misaligned.
fn offset_from<T>(slice: &[T], ptr: *const T) -> usize {
    assert!(ptr as usize % std::mem::align_of::<T>() == 0, "bad alignment");
    assert!(slice.as_ptr_range().contains(&ptr), "index oob");
    unsafe {
        ptr.offset_from(slice.as_ptr()) as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RAllocWrapper<'a>(pub (crate) *mut RAlloc<'a>);

unsafe impl<'a> Allocator for RAllocWrapper<'a> {
    fn allocate(&self, layout: std::alloc::Layout) -> Result<std::ptr::NonNull<[u8]>, std::alloc::AllocError> {
        let align = layout.align();
        let size = layout.size();

        let mut ptr = unsafe { &mut*self.0 }.malloc(size + align)?;
        // println!("Original allocation of {size} bytes at {:#x}", ptr.expose_addr());
        // make shure that align_offset actually is giving us a usefull answer
        // * NOTE: currently, when running this under MIRI with -Zmiri-symbolic-alignment-check, this will allways return usize::MAX.
        // *       nothing we can do about it, untill they add that functionality (it is intentional) but other than this its fine :shrug:
        assert_ne!(ptr.align_offset(align), usize::MAX);
        // make shure that the returned offset will not put us beyond the extra size of the allocation
        assert!(ptr.align_offset(align) < align);
        // offset the ptr
        ptr = unsafe { ptr.add(ptr.align_offset(align)) };
        let slice_ptr = slice_from_raw_parts_mut(ptr, size);
        let nonnull = NonNull::new(slice_ptr).unwrap();
        Ok(nonnull)
    }

    unsafe fn deallocate(&self, ptr: std::ptr::NonNull<u8>, _layout: std::alloc::Layout) {
        // let size = layout.size();
        let ptr = ptr.as_ptr();
        // println!("Deallocating {size} bytes, allocation at {:#x} (may be slignly misaligned)", ptr.expose_addr());
        (&mut*self.0).free(ptr);
    }
}
