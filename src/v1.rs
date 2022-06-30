

use core::{mem::size_of, ptr::{copy, write_bytes, self, NonNull}};
use std::alloc::{Layout, Allocator};

fn main() {
    let mut a = Alloc::new();
    // println!("{a:?}");
    println!("\
    start meta size: {START_META_SIZE} \
    end meta size: {END_META_SIZE} \
    mem size: {M_SIZE} \
    ");
    println!("read meta: {:#?}", unsafe { a.read_chunk_meta(0) });
    println!("current chunks: {:#?}", unsafe { a.chunks() });

    let a_ref = &AllocRef(&mut a as *mut _);
    let mut vec: Vec<String, _> = Vec::new_in(a_ref);
    vec.push(String::from("Hello, World!"));
    println!("{}", vec[0]);
}

#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct StartMeta {
    size: usize,
    in_use: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct EndMeta {
    size: usize,
}

const M_SIZE: usize = 1_024;
const START_META_SIZE: usize = size_of::<StartMeta>();
const END_META_SIZE: usize = size_of::<usize>();

#[derive(Debug)]
pub struct Alloc {
    memory: [u8; M_SIZE],
}

impl Alloc {
    pub fn new() -> Self {
        let mut s = Self {
            memory: [0; M_SIZE],
        };
        // M_SIZE - (START_META_SIZE + END_META_SIZE) -- the useable size of the initial memory
        #[allow(unused)]// ?????
        const USEABLE_SPACE: usize = M_SIZE - (START_META_SIZE + END_META_SIZE);
        // let start_meta = StartMeta {
        //     size: USEABLE_SPACE,
        //     in_use: false,
        // };
        // let end_meta = EndMeta {
        //     size: USEABLE_SPACE,
        // };
        // SAFTEY:
        // StartMeta and EndMeta are repr(C)
        // M_SIZE must be large enough to fit StartMeta and EndMeta
        debug_assert!(size_of::<StartMeta>() + size_of::<EndMeta>() < M_SIZE);
        // unsafe {
        //      copy(&start_meta as *const _ as *const u8, &mut s.memory as *mut u8, START_META_SIZE);
        //      copy(&end_meta as *const _ as *const u8, (&mut s.memory as *mut u8).offset((START_META_SIZE + USEABLE_SPACE) as isize), END_META_SIZE);
        // }

        unsafe { s.write_chunk_meta(0, USEABLE_SPACE, false) };
        s
    }

    pub unsafe fn malloc(&mut self, size: usize) -> *mut u8 {
        println!("Allocating size {size}");
        self.defrag();
        let mut all = self.chunks();
        all.sort_by(|a, b| a.1.size.cmp(&b.1.size));
        let mut all_that_fit = all.into_iter().filter(|i| i.1.size >= size).filter(|i| !i.1.in_use).collect::<Vec<_>>();
        if all_that_fit.is_empty() {
            panic!("Error: OOM");
        }
        let best = all_that_fit.remove(0);
        let extra = best.1.size - size;
        #[allow(unused)]
        const MIN_SIZE_TO_FRAGMENT: usize = 1024;// random number lol. DEFINITALLY should be a power of 2
        #[allow(unused)]
        const REAL_MIN_SIZE_TO_FRAGMENT: usize = MIN_SIZE_TO_FRAGMENT + START_META_SIZE + END_META_SIZE;
        if extra > MIN_SIZE_TO_FRAGMENT {
            self.split_chunk(best.0, size);
        }
        self.use_chunk(best.0)
    }

    pub unsafe fn free(&mut self, ptr: *mut u8) {
        let offset = self.offset_by_ptr(ptr);
        let (meta, _) = self.read_chunk_meta(offset);
        write_bytes(self.memory.as_mut_ptr().add(offset + START_META_SIZE), 0u8, meta.size);
        self.write_chunk_meta(offset, meta.size, false);
    }

    unsafe fn defrag(&mut self) {
        let mut chunks = self.chunks().into_iter().peekable();
        while let Some((offset, meta)) = chunks.next() {
            if !meta.in_use {
                let mut overall_size = meta.size;
                let mut count = 0usize;
                while let Some((_, StartMeta { size: s, in_use: true })) = chunks.peek() {
                    overall_size += START_META_SIZE + s + END_META_SIZE;
                    count += 1;
                }
                if count > 1 {
                    // merge all of the subchunks by overwriting them
                    self.write_chunk_meta(offset, overall_size, false);
                    // zero the memory
                    write_bytes(self.memory.as_mut_ptr().add(offset + START_META_SIZE), 0u8, overall_size);
                }
            }
        }
    }

    unsafe fn chunks(&self) -> Vec<(usize, StartMeta)> {
        let mut offset: usize = 0;
        let mut res = vec![];
        while offset < self.memory.len() {
            let meta = self.read_chunk_meta(offset);
            res.push((offset, meta.0));
            offset += START_META_SIZE + meta.0.size + END_META_SIZE;
        }
        res
    }

    /// Gets the offset of a chunk in self.memory from a pointer to that memory
    unsafe fn offset_by_ptr(&self, ptr: *const u8) -> usize {
        offset_from(&self.memory, ptr.sub(START_META_SIZE))
    }

    /// "uses" a chunk, setting it as taken and returning a pointer to its memory
    ///
    /// # Saftey
    ///
    /// - a valid chunk (StartMeta, [u8], EndMeta) must exist in self.memory, begining at offset, and that chunk **must not be in use!**
    /// - offset + START_META_SIZE must fit into isize
    ///
    unsafe fn use_chunk(&mut self, offset: usize) -> *mut u8 {
        let (StartMeta { size, in_use }, _) = self.read_chunk_meta(offset);
        debug_assert!(!in_use, "cannot use the same section of memory twice!");
        self.write_chunk_meta(offset, size, true);
        self.memory.as_mut_ptr().add(offset + START_META_SIZE)
    }

    /// Splits a chunk into two new chunks.
    ///
    /// the first of the new chunks exists at `offset`, with a size of `new_size`. Inforation about the second chunk can be found by
    /// getting the metadata of the chunk after the one at offset (after the operation)
    ///
    /// neither of the resulting chunks are in use
    ///
    /// # Saftey
    ///
    /// - a valid chunk (StartMeta, [u8], EndMeta) must exist in self.memory, begining at offset, and that chunk **must not be in use!**
    /// - offset must fit into isize
    /// - new_size must be less than (prev_size - (START_META_SIZE + END_META_SIZE))
    ///
    unsafe fn split_chunk(&mut self, offset: usize, new_size: usize) {
        let (csm, _) = self.read_chunk_meta(offset);
        self.write_chunk_meta(
            offset,
            new_size,
            false
        );
        let first_chunk_size = START_META_SIZE + new_size + END_META_SIZE;
        self.write_chunk_meta(
            offset + first_chunk_size,
            csm.size - (first_chunk_size - START_META_SIZE - END_META_SIZE),
            false
        );
    }

    /// # Saftey
    ///
    /// - a valid chunk (StartMeta, [u8], EndMeta) must exist in self.memory, begining at offset
    /// - offset must fit into isize
    ///
    unsafe fn read_chunk_meta(&self, offset: usize) -> (StartMeta, EndMeta) {
        // here, ptr::copy is used to avoid alignment issues, and Default::default() before writing is used to avoid (partially) uninit values
        // if it works, dont touch it lol

        let ptr = (&self.memory as *const u8).add(offset);
        let mut s = StartMeta::default();
         copy(ptr, &mut s as *mut _ as *mut u8, START_META_SIZE);
        let block_size = s.size;
        let mut e = EndMeta::default();
         copy(ptr.add(START_META_SIZE + block_size), &mut e as *mut _ as *mut u8, END_META_SIZE);
        (s, e)
    }

    /// # Saftey
    ///
    /// - offset must fit into isze
    /// - offset + START_META_SIZE + size + END_META_SIZE must be within the bounds of self.memory
    /// - this operation must not overwrite a currently in use chunk
    ///
    unsafe fn write_chunk_meta(&mut self, offset: usize, size: usize, in_use: bool) {
        let start_meta = StartMeta {
            size,
            in_use,
        };
        let end_meta = EndMeta {
            size,
        };
        // here, ptr::copy is used to avoid alignment issues
        // if it works, dont touch it lol
        let ptr = (&mut self.memory as *mut u8).add(offset);
         copy(&start_meta as *const _ as *const u8, ptr, START_META_SIZE);
         copy(&end_meta as *const _ as *const u8, ptr.add(START_META_SIZE + size), END_META_SIZE);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AllocRef(pub *mut Alloc);

unsafe impl std::alloc::Allocator for &AllocRef {
    fn allocate(&self, layout: std::alloc::Layout) -> Result<std::ptr::NonNull<[u8]>, std::alloc::AllocError> {
        // this allocator cannot do allignment, so we do some trickery to make it work
        let align = layout.align();
        let size = layout.size();

        //* old ver
        // let mask: usize = !(align-1);
        // let mem = unsafe { (*(*self).0).malloc(size + align - 1) };
        // let raw_ptr = ((mem.addr()+align-1) & mask) as *mut u8;
        // let ptr = ptr::from_raw_parts::<[u8]>(raw_ptr as *const (), size) as *mut [u8];
        // Ok(NonNull::new(ptr).unwrap())
        //* new ver
        println!("Allocating with size {size} and align {align}");
        todo!()
    }

    unsafe fn deallocate(&self, ptr: std::ptr::NonNull<u8>, layout: Layout) {
        todo!()
    }
}

/// Returns the offset of `ptr` into `slice`.
/// Panics if `ptr` points to a location outside the slice or is misaligned.
fn offset_from<T>(slice: &[T], ptr: *const T) -> usize {
    assert!(ptr as usize % std::mem::align_of::<T>() == 0);
    assert!(slice.as_ptr_range().contains(&ptr));
    unsafe {
        ptr.offset_from(slice.as_ptr()) as usize
    }
}

