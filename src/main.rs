#![feature(allocator_api)]
#![feature(once_cell)]

use numtoa::NumToA;
use ralloc::{allocator::RAlloc};
use na_print::{eprintln, print, println};
use std::{cell::UnsafeCell, alloc::GlobalAlloc, process, ptr::{NonNull, addr_of_mut}};

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

static mut IN_ALLOC: bool = false;
static mut MEMORY: [u8; 5_000] = [0u8; 5_000];
#[global_allocator]
static GLOBAL_ALLOC: GlobalAllocator = GlobalAllocator::new();

struct GlobalAllocator(UnsafeCell<RawGlobalAllocator>);
unsafe impl Sync for GlobalAllocator {}
impl GlobalAllocator {
    pub const fn new() -> Self {
        Self(UnsafeCell::new(RawGlobalAllocator::new()))
    }
}
unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        if IN_ALLOC {
            eprintln("Recursive allocation (alloc) occured, aborting!");
            process::abort();
        }
        IN_ALLOC = true;
        let mut buf = [0u8; 20];
        if DO_DEBUG_PRINTS {
            print("allocating ");
            print(layout.size().numtoa_str(10, &mut buf));
            println(" bytes");
        }
        let mut allocator = RAlloc::from_raw(NonNull::new_unchecked(addr_of_mut!(MEMORY)));
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
        IN_ALLOC = false;
        res
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        if IN_ALLOC {
            eprintln("Recursive allocation (dealloc) occured, aborting!");
            process::abort();
        }
        IN_ALLOC = true;
        if DO_DEBUG_PRINTS {
            println("deallocating");
        }
        let mut allocator = RAlloc::from_raw(NonNull::new_unchecked(addr_of_mut!(MEMORY)));
        allocator.allocator_compatable_free(NonNull::new_unchecked(ptr), layout);
        IN_ALLOC = false;
    }
}
struct RawGlobalAllocator { }
impl RawGlobalAllocator {
    pub const fn new() -> Self {
        Self { }
    }
}


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
