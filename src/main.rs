#![feature(allocator_api)]
#![feature(once_cell)]

use numtoa::NumToA;
use ralloc::{backing, wrappers, allocator::RAlloc};
use std::{sync::LazyLock, panic, cell::UnsafeCell, alloc::GlobalAlloc, process, ptr::{NonNull, addr_of_mut}};

static DEFAULT_HOOK: LazyLock<Box<dyn Fn(&panic::PanicInfo<'_>) + Sync + Send + 'static>> =
    LazyLock::new(|| {
        let hook = panic::take_hook();
        panic::set_hook(Box::new(|info| {
            if unsafe { IN_ALLOC } {
                na_print::eprintln("Panic while allocating!");
                if let Some(location) = info.location() {
                    na_print::eprint("panic occured at ");
                    na_print::eprint(location.file());
                    let mut buf = [0u8; 20];
                    na_print::eprint(":");
                    na_print::eprint(location.line().numtoa_str(10, buf.as_mut_slice()));
                    na_print::eprint(":");
                    na_print::eprint(location.column().numtoa_str(10, buf.as_mut_slice()));
                    na_print::eprintln("");
                }
                if let Some(&message) = info.payload().downcast_ref::<&str>() {
                    na_print::eprint("Message: ");
                    na_print::eprintln(message);
                }
                // attempt to call the normal handler anyway, although it will almost certantly fail
            }
            // Invoke the default handler, which prints the actual panic message and optionally a backtrace
            (*DEFAULT_HOOK)(info);
        }));
        hook
    });

pub fn install_ice_hook() {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "full");
    }
    LazyLock::force(&DEFAULT_HOOK);
}

static mut IN_ALLOC: bool = false;
static mut MEMORY: [u8; 1_000] = [0u8; 1_000];
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
            na_print::eprintln("Recursive allocation (alloc) occured, aborting!");
            process::abort();
        }
        IN_ALLOC = true;
        let mut buf = [0u8; 20];
        na_print::print("allocating ");
        na_print::print(layout.size().numtoa_str(10, &mut buf));
        na_print::println(" bytes");
        let mut allocator = RAlloc::from_raw(NonNull::new_unchecked(addr_of_mut!(MEMORY)));
        let res = allocator.allocator_compatable_malloc(layout).unwrap().as_ptr().cast::<u8>();
        na_print::print("Requested an allocation of alignment ");
        na_print::print(layout.align().numtoa_str(10, buf.as_mut_slice()));
        na_print::print(", got an allocation with an offset to that alignment of ");
        na_print::println((res as usize % layout.align()).numtoa_str(10, buf.as_mut_slice()));
        IN_ALLOC = false;
        res
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        if IN_ALLOC {
            na_print::eprintln("Recursive allocation (dealloc) occured, aborting!");
            process::abort();
        }
        IN_ALLOC = true;
        na_print::println("deallocating");
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
    install_ice_hook();
    // stack_backing_example();
}

#[allow(unused)]
fn stack_backing_example() {
    let mut backing = [0u8; 500];
    let backed = backing::BackedAllocator::new(backing.as_mut_slice()).unwrap();
    let alloc = wrappers::shared::RAllocShared::new(backed);

    let mut v = Vec::new_in(alloc.clone());
    v.push(*b"Hello, World!");
    println!("{}", String::from_utf8(v[0].to_vec()).unwrap());
}
