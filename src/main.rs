#![feature(allocator_api)]
#![feature(once_cell)]
#![feature(const_mut_refs)] // only needed for RAlloc::new_const
#![feature(strict_provenance)]// only needed for some debug prints in the Allocator impl

mod allocator;

use core::{mem::size_of, alloc::Allocator, ptr::{NonNull, slice_from_raw_parts_mut}};
use memmap2::{MmapOptions, MmapMut};
use std::{alloc::AllocError, fs::OpenOptions, io::Seek, path::Path, lazy::SyncOnceCell, cell::UnsafeCell};

use allocator::{RAlloc, RAllocWrapper};

fn main() {
    // let size = 1 * 1_000_000;// 1 mb
    // let mut handle = OpenOptions::new().create(true).read(true).write(true).open("pool.mmap").unwrap();
    // handle.seek(std::io::SeekFrom::Start(0)).unwrap();
    // handle.set_len(size as u64).unwrap();
    // let mut map = unsafe { MmapOptions::new().len(size).map_mut(&handle).unwrap() };
    // let mut alloc = RAlloc::new(&mut map[..]);

    let mut pool = [0u8; 2048];
    let mut alloc = RAlloc::new(pool.as_mut_slice());

    // println!("read meta: {:#?}", alloc.read_meta_at(0));
    // println!("current chunks:");
    // let mut offset = usize::MAX; //see docs for next_chunk
    // while let Some(n_offset) = alloc.next_chunk(offset) {
    //     offset = n_offset;
    //     let meta = alloc.read_meta_at(n_offset);
    //     println!("chunk: {:#?}", meta);
    // }
    let alloc_ptr = RAllocWrapper(&mut alloc as *mut _);
    let mut v: Vec<String, _> = Vec::new_in(alloc_ptr);
    v.push(String::from("Hello, World!"));
    println!("{}", v[0]);
}

// #[derive()]
// pub struct FileAllocator<'a> {
//     map: MmapMut,
//     allocator: RAlloc<'a>,
// }

// impl<'a> FileAllocator<'a> {
//     /// # Saftey
//     ///
//     /// this uses memmap2, and such
//     ///
//     /// > Safety: All file-backed memory map constructors are marked unsafe because of the potential for Undefined Behavior (UB) using
//     /// the map if the underlying file is subsequently modified, in or out of process. Applications must consider the risk and take
//     /// appropriate precautions when using file-backed maps. Solutions such as file permissions, locks or process-private (e.g. unlinked)
//     /// files exist but are platform specific and limited.
//     ///
//     #[forbid(unsafe_op_in_unsafe_fn)]
//     pub unsafe fn new<P: AsRef<Path>>(path: P, size: usize) -> std::io::Result<Self> {
//         let mut handle = OpenOptions::new().create(true).read(true).write(true).open(path)?;
//         handle.seek(std::io::SeekFrom::Start(0))?;
//         handle.set_len(size as u64)?;
//         // we talk about the saftey of this in this functions docs
//         let mut map = unsafe { MmapOptions::new().len(size).map_mut(&handle)? };
//         let len = map.as_ref().len();
//         // the lifetime of this is fine, since in the struct def the lifetime is the same lifetime as self
//         // the use-after-moved is also fine, because this is a memory map not a stack allocated object
//         let map_ref = unsafe { &mut*slice_from_raw_parts_mut(map.as_mut().as_mut_ptr(), len) };
//         Ok(Self {
//             map,
//             allocator: RAlloc::new(map_ref)
//         })
//     }
// }
