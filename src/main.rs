#![feature(allocator_api)]
#![feature(once_cell)]

use numtoa::NumToA;
use ralloc::allocator::RAlloc;
use na_print::{eprintln, print, println};
use core::{cell::RefCell, alloc::GlobalAlloc, ptr::NonNull};
use std::{process, sync::{LazyLock, Mutex}};

// // panic hook for debugging things (does not work)
// static DEFAULT_HOOK: LazyLock<Box<dyn Fn(&panic::PanicInfo<'_>) + Sync + Send + 'static>> =
//     LazyLock::new(|| {
//         let hook = panic::take_hook();
//         panic::set_hook(Box::new(|info| {
//             eprintln("Panic occured!");
//             if let Some(location) = info.location() {
//                 eprint("panic occured at ");
//                 eprint(location.file());
//                 let mut buf = [0u8; 20];
//                 eprint(":");
//                 eprint(location.line().numtoa_str(10, buf.as_mut_slice()));
//                 eprint(":");
//                 eprint(location.column().numtoa_str(10, buf.as_mut_slice()));
//                 eprintln("");
//             }
//             if let Some(&message) = info.payload().downcast_ref::<&str>() {
//                 eprint("Message: ");
//                 eprintln(message);
//             }
//             // Invoke the default handler, which prints the actual panic message and optionally a backtrace
//             (*DEFAULT_HOOK)(info);
//         }));
//         hook
//     });
// pub fn install_panic_hook() {
//     if std::env::var("RUST_BACKTRACE").is_err() {
//         std::env::set_var("RUST_BACKTRACE", "full");
//     }
//     LazyLock::force(&DEFAULT_HOOK);
// }

const DO_DEBUG_PRINTS: bool = false;

thread_local! {
    /// recursive allocation detection (must be thread local to remove false positives with multithreading)
    static IN_ALLOC: RefCell<bool> = RefCell::new(false);
}

#[global_allocator]
static GLOBAL_ALLOC: GlobalRalloc = GlobalRalloc::new();

pub struct GlobalRalloc(LazyLock<Mutex<RawGlobalRalloc>>);

impl GlobalRalloc {
    pub const fn new() -> Self {
        Self(LazyLock::new(|| {
            let v = Mutex::new(RawGlobalRalloc::new());
            v
        }))
    }
}

unsafe impl GlobalAlloc for GlobalRalloc {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        if IN_ALLOC.with(|x| {
            let mut r = x.borrow_mut();
            let v = *r;
            *r = true;
            v
        }) {
            eprintln("Recursive allocation (alloc) occured, aborting!");
            process::abort();
        }
        let guard = self.0.lock().unwrap();// not held by the current thread (see prev check)
        let mut buf = [0u8; 20];
        if DO_DEBUG_PRINTS {
            print("allocating ");
            print(layout.size().numtoa_str(10, &mut buf));
            println(" bytes");
        }
        let mut allocator = RAlloc::from_raw(guard.map);
        let res = match allocator.allocator_compatable_malloc(layout) {
            Ok(ptr) => ptr,
            Err(..) => {
                eprintln!("Memory allocation failed (most likely OOM)");
                eprintln!("Aborting");
                process::abort();
            }
        }.as_ptr().cast::<u8>();
        if DO_DEBUG_PRINTS {
            print("Requested an allocation of alignment ");
            print(layout.align().numtoa_str(10, buf.as_mut_slice()));
            print(", got an allocation with an offset to that alignment of ");
            println((res as usize % layout.align()).numtoa_str(10, buf.as_mut_slice()));
        }
        drop(guard);
        IN_ALLOC.with(|x| *x.borrow_mut() = false);
        res
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        if IN_ALLOC.with(|x| {
            let mut r = x.borrow_mut();
            let v = *r;
            *r = true;
            v
        }) {
            eprintln("Recursive allocation (dealloc) occured, aborting!");
            process::abort();
        }
        let guard = self.0.lock().unwrap();// not held by the current thread (see prev check)
        if DO_DEBUG_PRINTS {
            println("deallocating");
        }
        ptr.write_bytes(0b10101010u8, layout.size());
        let mut allocator = RAlloc::from_raw(guard.map);
        allocator.allocator_compatable_free(NonNull::new_unchecked(ptr), layout);
        drop(guard);
        IN_ALLOC.with(|x| *x.borrow_mut() = false);
    }
}

pub struct RawGlobalRalloc {
    map: NonNull<[u8]>
}

impl RawGlobalRalloc {
    pub fn new() -> Self {
        // use std::{fs::File, os::unix::prelude::FromRawFd, ptr::addr_of_mut};
        // println("Creating alloc");
        // let file = unsafe {
        //     let ptr = libc::fopen(b"mem.mmap\0".as_ptr() as _, b"rb+\0".as_ptr() as _);
        //     if ptr.is_null() {
        //         eprintln("Error opening file!");
        //         Err::<(), _>(std::io::Error::last_os_error()).unwrap();
        //     }
        //     let desc = libc::fileno(ptr);
        //     let file = File::from_raw_fd(desc);
        //     file.set_len(5_000).unwrap();
        //     file
        // };
        // let map = unsafe { memmap2::MmapOptions::new().map_mut(&file) }.unwrap().as_mut().into();
        // println("memory mapped");

        static mut MEMORY: [u8; 5_000] = [0u8; 5_000];
        let map = unsafe { MEMORY.as_mut_slice() }.into();

        if unsafe { RAlloc::new(map).is_none() } {
            eprintln!("Could not create allocator (memory is invalid / too small)");
            process::abort();
        }

        Self {
            map
        }
    }
}

unsafe impl Send for RawGlobalRalloc {} // MmapMut is Send, and it just contains a raw pointer

fn main() {
    println!("Hello, World!");
    // stack_backing_example();
}

// #[allow(unused)]
// fn stack_backing_example() {
//     use ralloc::{backing, wrappers};
//     let mut backing = [0u8; 500];
//     let backed = backing::BackedAllocator::new(backing.as_mut_slice()).unwrap();
//     let alloc = wrappers::shared::RAllocShared::new(backed);

//     let mut v = Vec::new_in(alloc.clone());
//     v.push(*b"Hello, World!");
//     println!("{}", String::from_utf8(v[0].to_vec()).unwrap());
// }
