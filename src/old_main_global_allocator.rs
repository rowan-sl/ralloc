// old main file with global alloc code

#![feature(allocator_api)]
// #![feature(once_cell)]
#![feature(const_mut_refs)] // only needed for RAlloc::new_const
// #![feature(const_slice_from_raw_parts)] // using Ralloc::new_const
#![feature(strict_provenance)]
#![feature(sync_unsafe_cell)]
#![feature(slice_ptr_get)]

// pub mod allocator;
pub mod allocator_v2;
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

use core::{alloc::GlobalAlloc, ptr::NonNull};
use std::alloc::Layout;
use std::{ptr, mem};
use std::{alloc::handle_alloc_error};
use std::cell::{SyncUnsafeCell, UnsafeCell};

use allocator_v2::RAlloc;

// use allocator_v2::RAlloc;

// struct GlobalRAlloc(SyncUnsafeCell<RAlloc>);

// const SIZE: usize = 64_000;
// static POOL: SyncUnsafeCell<[u8; SIZE]> = SyncUnsafeCell::new([0u8; SIZE]);
// // Saftey
// // (its not at all) (but only with threads)
// #[global_allocator]
// static ALLOC: GlobalRAlloc = unsafe { GlobalRAlloc(SyncUnsafeCell::new(if let Some(a) = RAlloc::new(&mut*slice_from_raw_parts_mut(POOL.get().cast::<u8>(), SIZE)) { a } else { unreachable_unchecked() })) };
// // FIXME: this IS UB (if used with threading)
// // i just want to see things go brrr
// unsafe impl GlobalAlloc for GlobalRAlloc {
//     unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
//         let s = &mut (*self.0.get());
//         match s.allocator_compatable_malloc(layout) {
//             Ok(v) => v.as_mut_ptr(),
//             Err(_) => handle_alloc_error(layout),
//         }
//     }

//     unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
//         let s = &mut (*self.0.get());
//         match NonNull::new(ptr) {
//             Some(ptr) => {
//                 s.allocator_compatable_free(ptr, layout)
//             }
//             None => {
//                 // GLHF
//                 std::process::abort();
//             }
//         }
//     }
// }

struct GlobalRAlloc(UnsafeCell<(bool /* initialized */, bool /* working (detects recursive allocation) */, bool /* do debug prints */)>, *mut [u8]);
unsafe impl Sync for GlobalRAlloc {}
impl GlobalRAlloc {
    pub const fn new(ptr: *mut[u8]) -> Self {
        Self(UnsafeCell::new((false, false, false)), ptr)
    }
}

unsafe impl GlobalAlloc for GlobalRAlloc {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        // let recursive_flag;
        // if (*self.0.get()).1 {
        //     recursive_flag = true;
        //     // std::process::abort();
        // } else {
        //     recursive_flag = false;
        //     (*self.0.get()).1 = true;
        //     if (*self.0.get()).2 {
        //         println!("allocating {:?} bytes", layout.size());
        //     }
        // }
        // let res = std::alloc::System::default().alloc(layout);
        if !(*self.0.get()).0 {
            let _ = RAlloc::new(self.1).unwrap_unchecked().into_raw();
            (*self.0.get()).0 = true;
        }
        let mut s = RAlloc::from_raw(self.1);
        let res = match s.allocator_compatable_malloc(layout) {
            Ok(v) => v.as_mut_ptr(),
            Err(_) => handle_alloc_error(layout),
        };
        let _ = s.into_raw();
        // (*self.0.get()).1 = recursive_flag;
        res
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        // let recursive_flag;
        // if (*self.0.get()).1 {
        //     recursive_flag = true;
        //     // std::process::abort();
        // } else {
        //     recursive_flag = false;
        //     (*self.0.get()).1 = true;
        //     if (*self.0.get()).2 {
        //         println!("deallocating {:?} bytes", layout.size());
        //     }
        // }
        // std::alloc::System::default().dealloc(ptr, layout);
        let mut s = RAlloc::from_raw(self.1);
        match NonNull::new(ptr) {
            Some(ptr) => {
                s.allocator_compatable_free(ptr, layout)
            }
            None => {
                // GLHF
                std::process::abort();
            }
        }
        let _ = s.into_raw();
        // (*self.0.get()).1 = recursive_flag;
    }
}

const SIZE: usize = 1_000;
static MEMORY_POOL: SyncUnsafeCell<[u8; SIZE]> = SyncUnsafeCell::new([0u8; SIZE]);
// #[global_allocator]
static GLOBAL_ALLOCATOR: GlobalRAlloc = GlobalRAlloc::new(MEMORY_POOL.get());

#[allow(dead_code)]
fn stack_backing_example() {
    let mut backing = [0u8; 500];
    let backed = backing::BackedAllocator::new(backing.as_mut_slice()).unwrap();
    let alloc = wrappers::shared::RAllocShared::new(backed);

    let mut v = Vec::new_in(alloc.clone());
    v.push(*b"Hello, World!");
    println!("{}", String::from_utf8(v[0].to_vec()).unwrap());
    // println!("foo");
    // let s = Box::new_in(*b"Hello, World!", alloc.clone());
    // println!("{}", String::from_utf8((*s).into()).unwrap());
}

#[allow(dead_code)]
fn global_alloc_example() {
    let mut s = String::from("Hello, World");
    s.push('!');
    println!("{}", s);
}

fn main() {
    // unsafe {(*GLOBAL_ALLOCATOR.0.get()).2 = true} // activate debug prints
    // println!("foo");
    stack_backing_example();
    // global_alloc_example();
    // unsafe {(*GLOBAL_ALLOCATOR.0.get()).2 = false}

    // unsafe {
    //     // let mut backing = [0u8; 500];
    //     // let mut allocator = RAlloc::new(backing.as_mut_slice() as _).unwrap();
    //     // let ptr = MEMORY_POOL.get();
    //     // let _ = RAlloc::new(ptr).unwrap().into_raw();
    //     #[derive(Debug, Clone)]
    //     struct SomeStruct {
    //         foo: u128,
    //         bar: String,
    //     }
    //     let data = SomeStruct {
    //         foo: 100,
    //         bar: String::from("test")
    //     };
    //     let layout = Layout::for_value(&data);

    //     // let mut alloc = RAlloc::from_raw(ptr);
    //     // let mem = alloc.allocator_compatable_malloc(layout).unwrap().cast::<SomeStruct>();
    //     // let _ = alloc.into_raw();
    //     let mem = GLOBAL_ALLOCATOR.alloc(layout) as *mut SomeStruct;
    //     let mem2 = GLOBAL_ALLOCATOR.alloc(layout) as *mut SomeStruct;

    //     ptr::write(mem, data.clone());
    //     let data1 = ptr::read(mem as *const SomeStruct);
    //     ptr::write(mem2, data.clone());
    //     let data2 = ptr::read(mem2 as *const SomeStruct);

    //     // let mut alloc = RAlloc::from_raw(ptr);
    //     // alloc.allocator_compatable_free(mem.cast::<u8>(), layout);
    //     // let _ = alloc.into_raw();
    //     GLOBAL_ALLOCATOR.dealloc(mem as *mut u8, layout);
    //     GLOBAL_ALLOCATOR.dealloc(mem2 as *mut u8, layout);

    //     println!("data1: {:?}\ndata2: {:?}", data1, data2);
    // }

    // let _ = 42;
    // let mut s = String::from("Hello, World");
    // s.push('!');
    // println!("{}", s);

    // let data = unsafe { ALLOC.alloc(Layout::for_value(&s)) };
    // unsafe { ALLOC.dealloc(data, Layout::for_value(&s)) };


    // let backed = backing::BackedAllocator::new(unsafe { backing::memmap::new_map("pool.mmap", 64_000) }.unwrap()).unwrap();
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
