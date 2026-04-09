<div>

<h1><code>wee_alloc_bw: Initiative to revive <a href="https://github.com/rustwasm/wee_alloc">wee_alloc</a></code></h1>
<p>
We made the following changes:
</p>

<ul>
  <li>Fix <code>extra_assertions</code> free-cell poisoning helpers to use raw allocation pointers for trailing payload access, avoiding Miri-reported UB from deriving tail writes through <code>&amp;FreeCell</code> references.</li>
  <li>Fix additional strict-provenance/Miri issues in intrusive free-list pointer handling by replacing integer-pointer tag manipulation with pointer address APIs and by keeping allocation/free/split paths on raw allocation-origin pointers end-to-end.</li>
  <li>Fixed long-lived fragmentation by implementing an eager splice-out strategy for deallocation, replacing the delayed-merge mechanism for the right neighbor.</li>
  <li>Fix a large-allocation free-list traversal bug that could leak memory when overlapping large allocations were freed and later reallocated instead of reusing both freed blocks <a href="https://github.com/rustwasm/wee_alloc/issues/106">106</a>.</li>
  <li>Fix an integer overflow in <code>LargeAllocPolicy::new_cell_for_free_list</code> that caused incorrect memory allocation and potential memory leaks when requesting very large sizes (for example, <code>usize::MAX - 8</code>) <a href="https://github.com/rustwasm/wee_alloc/issues/100">100</a>.</li>
  <li>Added <code>WeeAlloc::stats()</code> and <code>AllocStats</code> to allow monitoring the allocator's internal state (free list counts and total free bytes), gated behind the default-off <code>instrumentation</code> feature.</li>
  <li>Investigated memory leak cases listed in <a href="https://github.com/rustwasm/wee_alloc/issues/106">106</a>, but it does not appear to leak in the way described.</li>
</ul>
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
    <a href="https://github.com/bourumir-wyngs/wee_alloc_bw/actions/workflows/ci-miri.yml"><img src="https://github.com/bourumir-wyngs/wee_alloc_bw/actions/workflows/ci-miri.yml/badge.svg" alt="Miri CI" /></a>
    <a href="https://crates.io/crates/wee_alloc_bw"><img src="https://img.shields.io/crates/v/wee_alloc_bw.svg?style=flat-square" alt="Crates.io version" /></a>
    <a href="https://crates.io/crates/wee_alloc_bw"><img src="https://img.shields.io/crates/d/wee_alloc_bw.svg?style=flat-square" alt="Crates.io downloads" /></a>
    <a href="https://docs.rs/wee_alloc_bw"><img src="https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square" alt="docs.rs docs" /></a>
  </p>
</div>

{{readme}}
