#![feature(allocator_api)]
#![feature(once_cell)]
#![feature(const_mut_refs)] // only needed for RAlloc::new_const
#![feature(const_slice_from_raw_parts)] // using Ralloc::new_const
#![feature(strict_provenance)]// only needed for some debug prints in the Allocator impl
#![feature(sync_unsafe_cell)]
#![feature(slice_ptr_get)]

pub mod allocator;
pub mod wrappers;
pub mod backing;

// use core::{mem::size_of, alloc::Allocator, ptr::{NonNull, slice_from_raw_parts_mut}};
// use std::{alloc::{AllocError, Layout}, fs::OpenOptions, io::Seek, path::Path, cell::{UnsafeCell, RefCell}, sync::Mutex, ops::DerefMut, rc::Rc};

// use memmap2::{MmapOptions, MmapMut};

// const SIZE: usize = 64_000;
// static mut POOL: [u8; SIZE] = [0u8; SIZE];
// // static ALLOCATOR: Mutex<RAlloc<'static>> = parking_lot::const_mutex(unsafe { RAlloc::new_const(slice_from_raw_parts_mut(POOL.as_mut_ptr(), SIZE)) });

// fn us(map: MmapMut) -> MmapMut {
//     map
// }

use core::{alloc::GlobalAlloc, ptr::{NonNull, slice_from_raw_parts_mut}, cell::SyncUnsafeCell};
use std::{alloc::{handle_alloc_error, Layout}, cell::UnsafeCell};

use allocator::RAlloc;

struct GlobalRAlloc(SyncUnsafeCell<RAlloc<'static>>);

const SIZE: usize = 64_000;
static POOL: SyncUnsafeCell<[u8; SIZE]> = SyncUnsafeCell::new([0u8; SIZE]);
// Saftey
// (its not at all) (but only with threads)
#[global_allocator]
static ALLOC: GlobalRAlloc = unsafe { GlobalRAlloc(SyncUnsafeCell::new(RAlloc::new_const_uninit(slice_from_raw_parts_mut(POOL.get().cast(), SIZE)))) };

// FIXME: this IS UB (if used with threading)
// i just want to see things go brrr
unsafe impl GlobalAlloc for GlobalRAlloc {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let s = &mut (*self.0.get());
        if s.is_uninit() {
            s.init();
        }
        match s.allocator_compatable_malloc(layout) {
            Ok(v) => v.as_mut_ptr(),
            Err(_) => handle_alloc_error(layout),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        let s = &mut (*self.0.get());
        match NonNull::new(ptr) {
            Some(ptr) => {
                s.allocator_compatable_free(ptr, layout)
            }
            None => {
                // GLHF
                std::process::abort();
            }
        }
    }
}

fn main() {
    let _ = 42;
    // unsafe { (*ALLOC.0.get()).init() };
    let mut s = String::from("Hello, World");
    s.push('!');
    println!("{}", s);
    let data = unsafe { ALLOC.alloc(Layout::for_value(&s)) };
    unsafe { ALLOC.dealloc(data, Layout::for_value(&s)) };

    // let backed = backing::BackedAllocator::new(unsafe { backing::memmap::new_map("pool.mmap", 64_000) }.unwrap());
    // let alloc = wrappers::shared::RAllocShared::new(backed);

    // println!("{}", Box::new_in(*"Hello, World!", alloc.clone()));


    // let alloc: RAlloc<'static> = RAlloc::new(unsafe { POOL.as_mut_slice() });
    // let handle: RAllocWrapper<'static> = RAllocWrapper(Box::leak(Box::new(Mutex::new(alloc))));

    // let size = 1 * 1_000_000;// 1 mb
    // let mut handle = OpenOptions::new().create(true).read(true).write(true).open("pool.mmap").unwrap();
    // handle.seek(std::io::SeekFrom::Start(0)).unwrap();
    // handle.set_len(size as u64).unwrap();
    // let mut map = unsafe { MmapOptions::new().len(size).map_mut(&handle).unwrap() };
    // let ptr = slice_from_raw_parts_mut(<MmapMut as DerefMut>::deref_mut(&mut map).as_mut_ptr(), size);

    // let mut alloc = RAlloc::new(unsafe { &mut*ptr });

    // let mem = unsafe { alloc.allocator_compatable_malloc(Layout::for_value(&String::from("Hello, World!"))).unwrap().cast::<String>() };
    // unsafe { std::ptr::write(mem.as_ptr(), String::from("Hello, World!")) };

    // let map2 = us(map);

    // let read = unsafe { std::ptr::read(mem.as_ptr() as *const _) };
    // println!("{}", read);
    // unsafe { alloc.allocator_compatable_free(mem.cast::<u8>(), Layout::for_value(&String::from("Hello, World!"))) };

    // drop(alloc);
    // drop(map2);


    // let mut test = [0u8; 100];
    // let r: Rc<RefCell<&mut [u8]>> = Rc::new(RefCell::new(test.as_mut_slice()));

    // let mut alloc = RAlloc::new(&mut map[..]);

    // let mut pool = [0u8; 2048];
    // let mut alloc = RAlloc::new(pool.as_mut_slice());

    // println!("read meta: {:#?}", alloc.read_meta_at(0));
    // println!("current chunks:");
    // let mut offset = usize::MAX; //see docs for next_chunk
    // while let Some(n_offset) = alloc.next_chunk(offset) {
    //     offset = n_offset;
    //     let meta = alloc.read_meta_at(n_offset);
    //     println!("chunk: {:#?}", meta);
    // }
    // let alloc_ptr = RAllocWrapper(&mut alloc as *mut _);
    // let mut v: Vec<String, _> = Vec::new_in(alloc_ptr);
    // v.push(String::from("Hello, World!"));
    // println!("{}", v[0]);
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
