#![feature(allocator_api)]

extern crate histo;
#[macro_use]
extern crate quickcheck;
#[macro_use]
extern crate cfg_if;
extern crate rand;
extern crate wee_alloc_bw;

use quickcheck::{Arbitrary, Gen};
use rand::Rng;
use rand::RngExt;
use rand::SeedableRng;
#[cfg(feature = "allocator_api")]
use std::alloc::Allocator;
#[cfg(not(feature = "allocator_api"))]
use std::alloc::GlobalAlloc;
use std::alloc::Layout;
use std::f64;
use std::fs;
use std::io::Read;
use std::mem;
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    // Allocate this many bytes.
    Alloc(usize),

    // Free the n^th allocation we've made, or no-op if there it has already
    // been freed.
    Free(usize),
}

pub use Operation::*;

impl Operation {
    #[inline]
    fn arbitrary_alloc<R: Rng + ?Sized>(
        g: &mut R,
        active_allocs: &mut Vec<usize>,
        num_allocs: &mut usize,
    ) -> Self {
        active_allocs.push(*num_allocs);
        *num_allocs += 1;

        // Zero sized allocation 1/1000 times.
        if g.random_ratio(1, 1000) {
            return Alloc(0);
        }

        // XXX: Keep this synced with `wee_alloc_bw`.
        const NUM_SIZE_CLASSES: usize = 256;

        let max_small_alloc_size = (NUM_SIZE_CLASSES + 1) * mem::size_of::<usize>();

        // Do a large allocation with probability P = 1/20.
        if g.random_ratio(1, 20) {
            let n = g.random_range(1..10) * max_small_alloc_size
                + g.random_range(0..max_small_alloc_size);
            return Alloc(n);
        }

        // Small allocation.
        if g.random() {
            Alloc(g.random_range(12..17))
        } else {
            Alloc(max_small_alloc_size)
        }
    }

    #[inline]
    fn arbitrary_free<R: Rng + ?Sized>(g: &mut R, active_allocs: &mut Vec<usize>) -> Self {
        assert!(!active_allocs.is_empty());
        let i = g.random_range(0..active_allocs.len());
        Free(active_allocs.swap_remove(i))
    }
}

impl FromStr for Operation {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        if s.starts_with("Alloc(") && s.ends_with("),") {
            let start = "Alloc(".len();
            let end = s.len() - "),".len();
            let n: usize = s[start..end].parse().map_err(|_| ())?;
            return Ok(Alloc(n));
        }

        if s.starts_with("Free(") && s.ends_with("),") {
            let start = "Free(".len();
            let end = s.len() - "),".len();
            let idx: usize = s[start..end].parse().map_err(|_| ())?;
            return Ok(Free(idx));
        }

        Err(())
    }
}

#[derive(Debug, Clone)]
pub struct Operations(Vec<Operation>);

impl FromStr for Operations {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        let mut ops = vec![];
        for line in s.lines() {
            ops.push(line.parse()?);
        }
        Ok(Operations(ops))
    }
}

#[cfg(feature = "extra_assertions")]
const NUM_OPERATIONS: usize = 2_000;

#[cfg(not(feature = "extra_assertions"))]
const NUM_OPERATIONS: usize = 50_000;

impl Arbitrary for Operations {
    #[inline(never)]
    fn arbitrary(g: &mut Gen) -> Self {
        // Our tests are spending more time in the RNG under this `Arbitrary`
        // implementation than in the allocator. Speed things up a little bit
        // with this RNG.
        let mut g = rand::rngs::StdRng::from_seed(<[u8; 32]>::arbitrary(g));
        let g = &mut g;

        let mut num_allocs = 0;
        let mut active_allocs = vec![];
        let mut operations = Vec::with_capacity(NUM_OPERATIONS);

        for _ in 0..NUM_OPERATIONS {
            // Free with P = 1/4 so that we exercise more free list
            // refilling code paths due to the higher rates of
            // allocation.
            if !active_allocs.is_empty() && g.random_ratio(1, 4) {
                operations.push(Operation::arbitrary_free(g, &mut active_allocs));
            } else {
                operations.push(Operation::arbitrary_alloc(
                    g,
                    &mut active_allocs,
                    &mut num_allocs,
                ));
            }
        }

        operations.reserve_exact(active_allocs.len());
        while !active_allocs.is_empty() {
            operations.push(Operation::arbitrary_free(g, &mut active_allocs));
        }

        Operations(operations)
    }

    #[inline(never)]
    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        let ops = self.0.clone();
        let prefixes =
            (0..self.0.len()).map(move |i| Operations(ops.iter().cloned().take(i).collect()));

        let free_indices: Vec<_> = self
            .0
            .iter()
            .enumerate()
            .filter_map(|(i, op)| if let Free(_) = *op { Some(i) } else { None })
            .collect();

        let ops = self.0.clone();
        let without_frees = free_indices.into_iter().map(move |i| {
            Operations(
                ops.iter()
                    .enumerate()
                    .filter_map(|(j, op)| if i == j { None } else { Some(*op) })
                    .collect(),
            )
        });

        let alloc_indices: Vec<_> = self
            .0
            .iter()
            .enumerate()
            .filter_map(|(i, op)| if let Alloc(_) = *op { Some(i) } else { None })
            .collect();

        let ops = self.0.clone();
        let without_allocs = alloc_indices.clone().into_iter().map(move |i| {
            Operations(
                ops.iter()
                    .enumerate()
                    .filter_map(|(j, op)| {
                        if i == j {
                            None
                        } else if let Free(k) = *op {
                            if k == i {
                                None
                            } else if k > i {
                                Some(Free(k - 1))
                            } else {
                                Some(Free(k))
                            }
                        } else {
                            Some(*op)
                        }
                    })
                    .collect(),
            )
        });

        let ops = self.0.clone();
        let smaller_allocs = alloc_indices.into_iter().map(move |i| {
            Operations(
                ops.iter()
                    .enumerate()
                    .filter_map(|(j, op)| {
                        if i == j {
                            if let Alloc(size) = *op {
                                if size == 0 {
                                    None
                                } else {
                                    Some(Alloc(size / 2))
                                }
                            } else {
                                Some(*op)
                            }
                        } else {
                            Some(*op)
                        }
                    })
                    .collect(),
            )
        });

        // TODO: Merge allocs

        Box::new(
            prefixes
                .chain(without_frees)
                .chain(without_allocs)
                .chain(smaller_allocs),
        )
    }
}

impl Operations {
    pub fn run_single_threaded(&self) {
        static WEE: wee_alloc_bw::WeeAlloc = wee_alloc_bw::WeeAlloc::INIT;

        self.run_with_allocator(&WEE);
    }

    pub fn run_multi_threaded(ops0: Self, ops1: Self, ops2: Self, ops3: Self) {
        use std::thread;

        static WEE: wee_alloc_bw::WeeAlloc = wee_alloc_bw::WeeAlloc::INIT;

        let handle0 = thread::spawn(move || ops0.run_with_allocator(&WEE));
        let handle1 = thread::spawn(move || ops1.run_with_allocator(&WEE));
        let handle2 = thread::spawn(move || ops2.run_with_allocator(&WEE));
        let handle3 = thread::spawn(move || ops3.run_with_allocator(&WEE));

        handle0.join().expect("Thread 0 Failed");
        handle1.join().expect("Thread 1 Failed");
        handle2.join().expect("Thread 2 Failed");
        handle3.join().expect("Thread 3 Failed");
    }

    #[cfg(feature = "allocator_api")]
    pub fn run_with_allocator<A: Allocator>(&self, a: A) {
        let mut allocs = vec![];
        for op in self.0.iter().cloned() {
            match op {
                Alloc(n) => {
                    let layout = Layout::from_size_align(n, mem::size_of::<usize>()).unwrap();
                    allocs.push(match a.allocate(layout) {
                        Ok(ptr) => Some((ptr.cast::<u8>(), layout)),
                        Err(_) => None,
                    });
                }
                Free(idx) => {
                    if let Some(entry) = allocs.get_mut(idx) {
                        if let Some((ptr, layout)) = entry.take() {
                            unsafe {
                                a.deallocate(ptr, layout);
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(feature = "allocator_api"))]
    pub fn run_with_allocator<A: GlobalAlloc>(&self, a: &A) {
        use std::ptr::NonNull;
        let mut allocs = vec![];
        for op in self.0.iter().cloned() {
            match op {
                Alloc(n) => {
                    let layout = Layout::from_size_align(n, mem::size_of::<usize>()).unwrap();
                    let ptr = unsafe { a.alloc(layout.clone()) };
                    allocs.push(if !ptr.is_null() {
                        Some((NonNull::new(ptr).unwrap(), layout))
                    } else {
                        None
                    });
                }
                Free(idx) => {
                    if let Some(entry) = allocs.get_mut(idx) {
                        if let Some((ptr, layout)) = entry.take() {
                            unsafe {
                                a.dealloc(ptr.as_ptr(), layout);
                            }
                        }
                    }
                }
            }
        }
    }

    const NUM_BUCKETS: u64 = 20;

    pub fn size_histogram(&self) -> histo::Histogram {
        let mut histogram = histo::Histogram::with_buckets(Self::NUM_BUCKETS);
        for op in &self.0 {
            if let Alloc(n) = *op {
                let n = n as f64;
                let n = n.log2().round();
                histogram.add(n as u64);
            }
        }
        histogram
    }

    pub fn lifetime_histogram(&self) -> histo::Histogram {
        let mut histogram = histo::Histogram::with_buckets(Self::NUM_BUCKETS);
        for (i, op) in self.0.iter().enumerate() {
            if let Free(j) = *op {
                if j < i {
                    histogram.add((i - j) as u64);
                }
            }
        }
        histogram
    }

    pub fn read_trace(trace: &str) -> Self {
        let trace = Path::new(trace);
        let trace_dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/traces"));
        let mut file = fs::File::open(trace_dir.join(trace)).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        contents.parse().unwrap()
    }
}

////////////////////////////////////////////////////////////////////////////////

macro_rules! run_quickchecks {
    ($name:ident) => {
        #[test]
        fn $name() {
            fn single_threaded(ops: Operations) {
                ops.run_single_threaded();
            }

            quickcheck::QuickCheck::new()
                .tests(1)
                .quickcheck(single_threaded as fn(Operations) -> ());
        }
    };
}

// Let the test harness run each of our single threaded quickchecks concurrently
// with each other.
run_quickchecks!(quickchecks_0);
run_quickchecks!(quickchecks_1);
// Limit the extent of the stress testing for the limited-size static backend
cfg_if! {
    if #[cfg(not(feature = "static_array_backend"))] {
        run_quickchecks!(quickchecks_2);
        run_quickchecks!(quickchecks_3);
        run_quickchecks!(quickchecks_4);
        run_quickchecks!(quickchecks_5);
        run_quickchecks!(quickchecks_6);
        run_quickchecks!(quickchecks_7);
    }
}

#[test]
fn multi_threaded_quickchecks() {
    quickcheck::QuickCheck::new().tests(1).quickcheck(
        Operations::run_multi_threaded as fn(Operations, Operations, Operations, Operations) -> (),
    );
}

#[cfg(test)]
static ALIGNS: [usize; 10] = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512];

quickcheck! {
    fn single_allocation_with_size_and_align(size: usize, align: usize) -> () {
        let size = size % 65536;
        let align = ALIGNS[align % ALIGNS.len()];

        static WEE: wee_alloc_bw::WeeAlloc = wee_alloc_bw::WeeAlloc::INIT;
        let layout = Layout::from_size_align(size, align).unwrap();
        #[cfg(feature = "allocator_api")]
        let _ = (&WEE).allocate(layout);
        #[cfg(not(feature = "allocator_api"))]
        let _ = unsafe { WEE.alloc(layout) };
    }
}

////////////////////////////////////////////////////////////////////////////////

macro_rules! test_trace {
    ($name:ident, $trace:expr) => {
        #[test]
        fn $name() {
            let ops = Operations::read_trace($trace);
            ops.run_single_threaded();
        }
    };
}

test_trace!(test_trace_cpp_demangle, "../traces/cpp-demangle.trace");
test_trace!(test_trace_dogfood, "../traces/dogfood.trace");
test_trace!(test_trace_ffmpeg, "../traces/ffmpeg.trace");
test_trace!(test_trace_find, "../traces/find.trace");
test_trace!(test_trace_gcc_hello, "../traces/gcc-hello.trace");
test_trace!(
    test_trace_grep_random_data,
    "../traces/grep-random-data.trace"
);
test_trace!(test_trace_grep_recursive, "../traces/grep-recursive.trace");
test_trace!(test_trace_ls, "../traces/ls.trace");
test_trace!(test_trace_source_map, "../traces/source-map.trace");

////////////////////////////////////////////////////////////////////////////////

#[test]
fn regression_test_0() {
    Operations(vec![Alloc(1)]).run_single_threaded();
}

#[test]
fn regression_test_1() {
    Operations(vec![Alloc(1414), Free(0), Alloc(1414), Free(1)]).run_single_threaded();
}

#[test]
fn regression_test_2() {
    Operations(vec![Alloc(168), Free(0), Alloc(0), Alloc(168), Free(2)]).run_single_threaded();
}

#[test]
fn regression_test_3() {
    Operations(vec![Alloc(13672), Free(0), Alloc(1)]).run_single_threaded();
}

#[test]
fn allocate_size_zero() {
    Operations(
        std::iter::repeat_n(Alloc(0), 1000)
            .chain((0..1000).map(Free))
            .collect(),
    )
    .run_single_threaded();
}

#[test]
fn allocate_many_small() {
    Operations(
        std::iter::repeat_n(Alloc(16 * mem::size_of::<usize>()), 100)
            .chain((0..100).map(Free))
            .chain(std::iter::repeat_n(
                Alloc(256 * mem::size_of::<usize>()),
                100,
            ))
            .chain((0..100).map(|i| Free(i + 100)))
            .collect(),
    )
    .run_single_threaded();
}

#[test]
fn allocate_many_large() {
    Operations(
        std::iter::repeat_n(Alloc(257 * mem::size_of::<usize>()), 100)
            .chain((0..100).map(Free))
            .chain(std::iter::repeat_n(
                Alloc(1024 * mem::size_of::<usize>()),
                100,
            ))
            .chain((0..100).map(|i| Free(i + 100)))
            .collect(),
    )
    .run_single_threaded();
}

////////////////////////////////////////////////////////////////////////////////

// Tests taken from
// https://github.com/alexcrichton/dlmalloc-rs/blob/master/tests/smoke.rs and
// modified.

#[test]
fn smoke() {
    static WEE: wee_alloc_bw::WeeAlloc = wee_alloc_bw::WeeAlloc::INIT;
    let a = &WEE;
    unsafe {
        let layout = Layout::new::<u8>();
        #[cfg(feature = "allocator_api")]
        let ptr = a
            .allocate(layout)
            .expect("Should be able to alloc a fresh Layout clone")
            .cast::<u8>();
        #[cfg(not(feature = "allocator_api"))]
        let ptr = {
            let p = a.alloc(layout);
            assert!(!p.is_null());
            std::ptr::NonNull::new(p).unwrap()
        };

        {
            let ptr = ptr.as_ptr();
            *ptr = 9;
            assert_eq!(*ptr, 9);
        }
        #[cfg(feature = "allocator_api")]
        a.deallocate(ptr, layout);
        #[cfg(not(feature = "allocator_api"))]
        a.dealloc(ptr.as_ptr(), layout);

        #[cfg(feature = "allocator_api")]
        let ptr = a
            .allocate(layout)
            .expect("Should be able to alloc from a second clone")
            .cast::<u8>();
        #[cfg(not(feature = "allocator_api"))]
        let ptr = {
            let p = a.alloc(layout);
            assert!(!p.is_null());
            std::ptr::NonNull::new(p).unwrap()
        };

        {
            let ptr = ptr.as_ptr();
            *ptr = 10;
            assert_eq!(*ptr, 10);
        }
        #[cfg(feature = "allocator_api")]
        a.deallocate(ptr, layout);
        #[cfg(not(feature = "allocator_api"))]
        a.dealloc(ptr.as_ptr(), layout);
    }
}

#[test]
fn cannot_alloc_max_usize() {
    static WEE: wee_alloc_bw::WeeAlloc = wee_alloc_bw::WeeAlloc::INIT;
    let a = &WEE;
    assert!(
        Layout::from_size_align(usize::MAX, 1).is_err(),
        "modern Rust rejects oversized `Layout`s before allocation"
    );

    let layout = Layout::from_size_align(isize::MAX as usize, 1)
        .expect("should be able to create the largest valid `Layout`");
    #[cfg(feature = "allocator_api")]
    {
        let result = a.allocate(layout);
        assert!(result.is_err());
    }
    #[cfg(not(feature = "allocator_api"))]
    unsafe {
        let result = a.alloc(layout);
        assert!(result.is_null());
    }
}

// This takes too long with our extra assertion checks enabled,
// and the fixed-sized static array backend is too small.
#[test]
#[cfg(not(any(feature = "extra_assertions", feature = "static_array_backend")))]
fn stress() {
    use std::cmp;

    let a = &wee_alloc_bw::WeeAlloc::INIT;
    let mut rng = rand::rng();
    let mut ptrs = Vec::new();
    unsafe {
        for _ in 0..100_000 {
            let free = ptrs.len() > 0
                && ((ptrs.len() < 1_000 && rng.random_ratio(1, 3)) || rng.random());
            if free {
                let idx = rng.random_range(0..ptrs.len());
                let (ptr, layout): (std::ptr::NonNull<u8>, Layout) = ptrs.swap_remove(idx);
                #[cfg(feature = "allocator_api")]
                a.deallocate(ptr, layout);
                #[cfg(not(feature = "allocator_api"))]
                a.dealloc(ptr.as_ptr(), layout);
                continue;
            }

            if ptrs.len() > 0 && rng.random_ratio(1, 100) {
                let idx = rng.random_range(0..ptrs.len());
                let (ptr, old): (std::ptr::NonNull<u8>, Layout) = ptrs.swap_remove(idx);
                let new = if rng.random() {
                    Layout::from_size_align(
                        rng.random_range(old.size()..old.size() * 2),
                        old.align(),
                    )
                        .unwrap()
                } else if old.size() > 10 {
                    Layout::from_size_align(
                        rng.random_range(old.size() / 2..old.size()),
                        old.align(),
                    )
                        .unwrap()
                } else {
                    continue;
                };
                let mut tmp = Vec::new();
                for i in 0..cmp::min(old.size(), new.size()) {
                    tmp.push(*(ptr.as_ptr() as *mut u8).offset(i as isize));
                }

                // Re-implement realloc for tests if needed or just use alloc + copy + dealloc
                #[cfg(feature = "allocator_api")]
                let new_ptr = {
                    let np = a.allocate(new).unwrap().cast::<u8>();
                    std::ptr::copy_nonoverlapping(
                        ptr.as_ptr(),
                        np.as_ptr(),
                        cmp::min(old.size(), new.size()),
                    );
                    a.deallocate(ptr, old);
                    np
                };
                #[cfg(not(feature = "allocator_api"))]
                let new_ptr = {
                    let np = a.realloc(ptr.as_ptr(), old, new.size());
                    assert!(!np.is_null());
                    std::ptr::NonNull::new(np).unwrap()
                };

                for (i, byte) in tmp.iter().enumerate() {
                    assert_eq!(*byte, *(new_ptr.as_ptr() as *mut u8).offset(i as isize));
                }
                ptrs.push((new_ptr, new));
                continue;
            }

            let size = if rng.random() {
                rng.random_range(1..128)
            } else {
                rng.random_range(1..128 * 1024)
            };
            let align = 1 << rng.random_range(0..3);

            let zero = rng.random_ratio(1, 50);
            let layout = Layout::from_size_align(size, align).unwrap();

            #[cfg(feature = "allocator_api")]
            let ptr = if zero {
                a.allocate_zeroed(layout.clone()).unwrap().cast::<u8>()
            } else {
                a.allocate(layout.clone()).unwrap().cast::<u8>()
            };
            #[cfg(not(feature = "allocator_api"))]
            let ptr = {
                let p = if zero {
                    a.alloc_zeroed(layout.clone())
                } else {
                    a.alloc(layout.clone())
                };
                assert!(!p.is_null());
                std::ptr::NonNull::new(p).unwrap()
            };

            for i in 0..layout.size() {
                if zero {
                    assert_eq!(*(ptr.as_ptr() as *mut u8).offset(i as isize), 0);
                }
                *(ptr.as_ptr() as *mut u8).offset(i as isize) = 0xce;
            }
            ptrs.push((ptr, layout));
        }
    }
}
