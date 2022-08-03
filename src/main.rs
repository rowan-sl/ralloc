#![feature(allocator_api)]

use ralloc::{backing, wrappers};

fn main() {
    stack_backing_example();
}

fn stack_backing_example() {
    let mut backing = [0u8; 500];
    let backed = backing::BackedAllocator::new(backing.as_mut_slice()).unwrap();
    let alloc = wrappers::shared::RAllocShared::new(backed);

    let mut v = Vec::new_in(alloc.clone());
    v.push(*b"Hello, World!");
    println!("{}", String::from_utf8(v[0].to_vec()).unwrap());
}
