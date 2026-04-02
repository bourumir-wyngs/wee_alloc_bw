use super::{AllocErr};
use const_init::ConstInit;
#[cfg(feature = "extra_assertions")]
use core::cell::Cell;
use core::ptr::{self, NonNull};
use memory_units::{Bytes, Pages};
use spin::Mutex;

const SCRATCH_LEN_BYTES: usize = include!(concat!(
    env!("OUT_DIR"),
    "/wee_alloc_static_array_backend_size_bytes.txt"
));

#[repr(align(4096))]
struct ScratchHeap([u8; SCRATCH_LEN_BYTES]);

static mut SCRATCH_HEAP: ScratchHeap = ScratchHeap([0; SCRATCH_LEN_BYTES]);
static mut OFFSET: Mutex<usize> = Mutex::new(0);

pub(crate) unsafe fn alloc_pages(pages: Pages) -> Result<NonNull<u8>, AllocErr> {
    let bytes: Bytes = pages.into();
    #[allow(static_mut_refs)]
    let mut offset = OFFSET.lock();
    let start = *offset;
    let end = bytes.0.checked_add(start).ok_or_else(AllocErr::new)?;
    if end <= SCRATCH_LEN_BYTES {
        let ptr = ptr::addr_of_mut!(SCRATCH_HEAP.0).cast::<u8>().add(start);
        *offset = end;
        NonNull::new(ptr).ok_or_else(AllocErr::new)
    } else {
        Err(AllocErr::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_units::Pages;

    #[test]
    fn test_alloc_pages_exact_fit() {
        // We want to test if allocating EXACTLY the remaining capacity of the scratch heap works.
        // The bug was that `end < SCRATCH_LEN_BYTES` was used instead of `end <= SCRATCH_LEN_BYTES`.
        
        // Lock offset and reset it to test from a clean state.
        #[allow(static_mut_refs)]
        let mut offset = unsafe { OFFSET.lock() };
        *offset = 0;
        
        let scratch_len = SCRATCH_LEN_BYTES;
        
        // Calculate how many pages fit into the scratch heap.
        // We know from the library that 1 Page = 65536 bytes.
        // memory_units::Pages(1).into() as Bytes is 65536.
        let bytes_per_page = 65536;
        let max_pages = scratch_len / bytes_per_page;
        
        if max_pages == 0 {
            // Scratch heap too small for even one page, skip.
            return;
        }
        
        // We must drop the lock before calling alloc_pages!
        drop(offset);
        
        // We allocate all available pages. 
        // This will bring the internal OFFSET to exactly SCRATCH_LEN_BYTES if max_pages * bytes_per_page == scratch_len,
        // which tests the exact-fit bug.
        let result = unsafe { alloc_pages(Pages(max_pages)) };
        
        assert!(result.is_ok(), "Exact fit allocation failed! scratch_len: {}, max_pages: {}", scratch_len, max_pages);
    }
}

pub(crate) struct Exclusive<T> {
    inner: Mutex<T>,

    #[cfg(feature = "extra_assertions")]
    in_use: Cell<bool>,
}

impl<T: ConstInit> ConstInit for Exclusive<T> {
    const INIT: Self = Exclusive {
        inner: Mutex::new(T::INIT),

        #[cfg(feature = "extra_assertions")]
        in_use: Cell::new(false),
    };
}

extra_only! {
    fn assert_not_in_use<T>(excl: &Exclusive<T>) {
        assert!(!excl.in_use.get(), "`Exclusive<T>` is not re-entrant");
    }
}

extra_only! {
    fn set_in_use<T>(excl: &Exclusive<T>) {
        excl.in_use.set(true);
    }
}

extra_only! {
    fn set_not_in_use<T>(excl: &Exclusive<T>) {
        excl.in_use.set(false);
    }
}

impl<T> Exclusive<T> {
    /// Get exclusive, mutable access to the inner value.
    ///
    /// # Safety
    ///
    /// It is the callers' responsibility to ensure that `f` does not re-enter
    /// this method for this `Exclusive` instance.
    //
    // XXX: If we don't mark this function inline, then it won't be, and the
    // code size also blows up by about 200 bytes.
    #[inline]
    pub(crate) unsafe fn with_exclusive_access<F, U>(&self, f: F) -> U
    where
        for<'x> F: FnOnce(&'x mut T) -> U,
    {
        let mut guard = self.inner.lock();
        assert_not_in_use(self);
        set_in_use(self);
        let result = f(&mut guard);
        set_not_in_use(self);
        result
    }
}
