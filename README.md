# ralloc

Archived on <https://git.fawkes.io/heap/ralloc>

poorly implemented heap allocator. do not use. like really. dont do it.

This is based off of what I read in [this](https://gee.cs.oswego.edu/dl/html/malloc.html) and a few other articles

can use stack memory, or memory mapped files as backing. (or whatever, really)

## no-std support

use `default-features = false` to disable standard library features. (this disables the `std` feature)

support for some features that require `alloc` (lol) but not `std` can be enabled with the `alloc` feature
