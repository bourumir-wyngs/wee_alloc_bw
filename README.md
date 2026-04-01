<div>

<h1><code>wee_alloc_bw: Initiative to revive <a href="https://github.com/rustwasm/wee_alloc">wee_alloc</a></code></h1>
<p>
wee_alloc is a widely used allocator in the Rust WebAssembly ecosystem, but the original project is no longer maintained and has outstanding issues. This fork revives the project with a focus on correctness, reliability, and active maintenance. We prioritize building a comprehensive test suite and addressing known issues before introducing changes. Maintaining API compatibility with the original crate is a key goal.
</p>

  <strong>The <u>W</u>asm-<u>E</u>nabled, <u>E</u>lfin Allocator</strong>

  <p>
    <a href="https://github.com/bourumir-wyngs/wee_alloc_bw/actions/workflows/ci-linux.yml"><img src="https://github.com/bourumir-wyngs/wee_alloc_bw/actions/workflows/ci-linux.yml/badge.svg" alt="Linux CI" /></a>
    <a href="https://github.com/bourumir-wyngs/wee_alloc_bw/actions/workflows/ci-macos.yml"><img src="https://github.com/bourumir-wyngs/wee_alloc_bw/actions/workflows/ci-macos.yml/badge.svg" alt="macOS CI" /></a>
    <a href="https://github.com/bourumir-wyngs/wee_alloc_bw/actions/workflows/ci-windows.yml"><img src="https://github.com/bourumir-wyngs/wee_alloc_bw/actions/workflows/ci-windows.yml/badge.svg" alt="Windows CI" /></a>
    <a href="https://github.com/bourumir-wyngs/wee_alloc_bw/actions/workflows/ci-wasm.yml"><img src="https://github.com/bourumir-wyngs/wee_alloc_bw/actions/workflows/ci-wasm.yml/badge.svg" alt="Wasm CI" /></a>
    <a href="https://github.com/bourumir-wyngs/wee_alloc_bw/actions/workflows/ci-api-compat.yml"><img src="https://github.com/bourumir-wyngs/wee_alloc_bw/actions/workflows/ci-api-compat.yml/badge.svg" alt="wee_alloc 0.4.5 compatible" /></a>
    <a href="https://crates.io/crates/wee_alloc_bw"><img src="https://img.shields.io/crates/v/wee_alloc_bw.svg?style=flat-square" alt="Crates.io version" /></a>
    <a href="https://crates.io/crates/wee_alloc_bw"><img src="https://img.shields.io/crates/d/wee_alloc_bw.svg?style=flat-square" alt="Crates.io downloads" /></a>
    <a href="https://docs.rs/wee_alloc_bw"><img src="https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square" alt="docs.rs docs" /></a>
  </p>
</div>


### About

`wee_alloc_bw`: The **W**asm-**E**nabled, **E**lfin Allocator.

- **Elfin, i.e. small:** Generates less than a kilobyte of uncompressed
  WebAssembly code. Doesn't pull in the heavy panicking or formatting
  infrastructure. `wee_alloc_bw` won't bloat your `.wasm` download size on the Web.

- **WebAssembly enabled:** Designed for the `wasm32-unknown-unknown` target and
  `#![no_std]`.

`wee_alloc_bw` is focused on targeting WebAssembly, producing a small `.wasm` code
size, and having a simple, correct implementation. It is geared towards code
that makes a handful of initial dynamically sized allocations, and then performs
its heavy lifting without any further allocations. This scenario requires *some*
allocator to exist, but we are more than happy to trade allocation performance
for small code size. In contrast, `wee_alloc_bw` would be a poor choice for a
scenario where allocation is a performance bottleneck.

Although WebAssembly is the primary target, `wee_alloc_bw` also has an `mmap` based
implementation for unix systems, a `VirtualAlloc` implementation for Windows,
and a static array-based backend for OS-independent environments. This enables
testing `wee_alloc_bw`, and code using `wee_alloc_bw`, without a browser or
WebAssembly engine.

`wee_alloc_bw` compiles on stable Rust 1.33 and newer.

- [Using `wee_alloc_bw` as the Global Allocator](#using-wee_alloc_bw-as-the-global-allocator)
- [`cargo` Features](#cargo-features)
- [Implementation Notes and Constraints](#implementation-notes-and-constraints)
- [License](#license)
- [Contribution](#contribution)

### Using `wee_alloc_bw` as the Global Allocator

```rust
extern crate wee_alloc_bw;

// Use `wee_alloc_bw` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc_bw::WeeAlloc = wee_alloc_bw::WeeAlloc::INIT;
```

### `cargo` Features

- **size_classes**: On by default. Use size classes for smaller allocations to
  provide amortized *O(1)* allocation for them. Increases uncompressed `.wasm`
  code size by about 450 bytes (up to a total of ~1.2K). It is however slow
  without this feature, only turn off if there are really only few allocations.

- **extra_assertions**: Enable various extra, expensive integrity assertions and
  defensive mechanisms, such as poisoning freed memory. This incurs a large
  runtime overhead. It is useful when debugging a use-after-free or `wee_alloc_bw`
  itself.

- **static_array_backend**: Force the use of an OS-independent backing
  implementation with a global maximum size fixed at compile time.  Suitable for
  deploying to non-WASM/Unix/Windows `#![no_std]` environments, such as on
  embedded devices with esoteric or effectively absent operating systems. The
  size defaults to 32 MiB (33554432 bytes), and may be controlled at build-time
  by supplying an optional environment variable to cargo,
  `WEE_ALLOC_STATIC_ARRAY_BACKEND_BYTES`. Note that this feature requires
  nightly Rust.

- **nightly**: Enable usage of nightly-only Rust features, such as implementing
  the `Alloc` trait (not to be confused with the stable `GlobalAlloc` trait!)

### Implementation Notes and Constraints

- `wee_alloc_bw` imposes two words of overhead on each allocation for maintaining
  its internal free lists.

- Deallocation is an *O(1)* operation if neither neighbor is free, or only the left neighbor is free. It is *O(n)* if the right neighbor is free.

- `wee_alloc_bw` will never return freed pages to the WebAssembly engine /
  operating system. Currently, WebAssembly can only grow its heap, and can never
  shrink it. All allocated pages are indefinitely kept in `wee_alloc_bw`'s internal
  free lists for potential future allocations, even when running on unix
  targets.

- `wee_alloc_bw` uses a simple, first-fit free list implementation. This means that
  allocation is an *O(n)* operation.

  Using the `size_classes` feature enables extra free lists dedicated to small
  allocations (less than or equal to 256 words). The size classes' free lists
  are populated by allocating large blocks from the main free list, providing
  amortized *O(1)* allocation time. Allocating from the size classes' free lists
  uses the same first-fit routines that allocating from the main free list does,
  which avoids introducing more code bloat than necessary.

Finally, here is a diagram giving an overview of `wee_alloc_bw`'s implementation:

```
+------------------------------------------------------------------------------+
| WebAssembly Engine / Operating System                                        |
+------------------------------------------------------------------------------+
                   |
                   |
                   | 64KiB Pages
                   |
                   V
+------------------------------------------------------------------------------+
| Main Free List                                                               |
|                                                                              |
|          +------+     +------+     +------+     +------+                     |
| Head --> | Cell | --> | Cell | --> | Cell | --> | Cell | --> ...             |
|          +------+     +------+     +------+     +------+                     |
|                                                                              |
+------------------------------------------------------------------------------+
                   |                                    |            ^
                   |                                    |            |
                   | Large Blocks                       |            |
                   |                                    |            |
                   V                                    |            |
+---------------------------------------------+         |            |
| Size Classes                                |         |            |
|                                             |         |            |
|             +------+     +------+           |         |            |
| Head(1) --> | Cell | --> | Cell | --> ...   |         |            |
|             +------+     +------+           |         |            |
|                                             |         |            |
|             +------+     +------+           |         |            |
| Head(2) --> | Cell | --> | Cell | --> ...   |         |            |
|             +------+     +------+           |         |            |
|                                             |         |            |
| ...                                         |         |            |
|                                             |         |            |
|               +------+     +------+         |         |            |
| Head(256) --> | Cell | --> | Cell | --> ... |         |            |
|               +------+     +------+         |         |            |
|                                             |         |            |
+---------------------------------------------+         |            |
                      |            ^                    |            |
                      |            |                    |            |
          Small       |      Small |        Large       |      Large |
          Allocations |      Frees |        Allocations |      Frees |
                      |            |                    |            |
                      |            |                    |            |
                      |            |                    |            |
                      |            |                    |            |
                      |            |                    |            |
                      V            |                    V            |
+------------------------------------------------------------------------------+
| User Application                                                             |
+------------------------------------------------------------------------------+
```

### License

Licensed under the [Mozilla Public License 2.0](https://www.mozilla.org/en-US/MPL/2.0/).

[TL;DR?](https://choosealicense.com/licenses/mpl-2.0/)

> Permissions of this weak copyleft license are conditioned on making available
> source code of licensed files and modifications of those files under the same
> license (or in certain cases, one of the GNU licenses). Copyright and license
> notices must be preserved. Contributors provide an express grant of patent
> rights. However, a larger work using the licensed work may be distributed
> under different terms and without source code for files added in the larger
> work.

### Contribution

See
[CONTRIBUTING.md](https://github.com/rustwasm/wee_alloc/blob/master/CONTRIBUTING.md)
for hacking!

