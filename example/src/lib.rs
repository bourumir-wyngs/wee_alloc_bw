//! An example of what using `wee_alloc_bw` as the global allocator in a
//! `#![no_std]` crate targeting `wasm32-unknown-unknown` looks like!

// First, some boilerplate and set up //////////////////////////////////////////

// We aren't using the standard library.
#![no_std]
// Replacing the allocator and using the `alloc` crate are still unstable.
#![allow(internal_features)]
#![feature(lang_items, alloc_error_handler)]

#[macro_use]
extern crate cfg_if;

extern crate alloc;
extern crate wee_alloc_bw;

#[cfg(not(any(test, feature = "use_std")))]
#[inline(always)]
fn abort() -> ! {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        core::arch::wasm32::unreachable()
    }

    #[cfg(not(target_arch = "wasm32"))]
    loop {}
}

// Use `wee_alloc_bw` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc_bw::WeeAlloc = wee_alloc_bw::WeeAlloc::INIT;

// Need to provide a tiny `panic` implementation for `#![no_std]`.
// This translates into an `unreachable` instruction that will
// raise a `trap` the WebAssembly execution if we panic at runtime.
cfg_if! {
    if #[cfg(not(any(test, feature = "use_std")))] {
        #[panic_handler]
        pub fn panic(_info: &::core::panic::PanicInfo) -> ! {
            abort()
        }

        #[alloc_error_handler]
        pub fn oom(_: ::core::alloc::Layout) -> ! {
            abort()
        }
    }
}

// Needed for non-wasm targets.
cfg_if! {
    if #[cfg(all(not(target_arch = "wasm32"), not(any(test, feature = "use_std"))))] {
        #[lang = "eh_personality"]
        pub extern "C" fn eh_personality() {}
    }
}

// Now, use the allocator via `alloc` types! ///////////////////////////////////

use alloc::boxed::Box;

// Box a `u8`!
#[no_mangle]
pub extern "C" fn hello() -> *mut u8 {
    Box::into_raw(Box::new(42))
}

// Free a `Box<u8>` that we allocated earlier!
#[no_mangle]
/// # Safety
///
/// `ptr` must have been returned by [`hello`] and must not have been freed
/// already.
pub unsafe extern "C" fn goodbye(ptr: *mut u8) {
    let _ = Box::from_raw(ptr);
}
