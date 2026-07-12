# stumpalo

<img alt="logo" src="https://codeberg.org/414owen/stumpalo/raw/main/logo/stumpy.svg" height="100">

[![repo badge](https://img.shields.io/badge/codeberg-repo-blue?logo=codeberg&logoColor=white)](https://codeberg.org/414owen/stumpalo)
[![docs badge](https://img.shields.io/docsrs/stumpalo?logo=wikibooks)](https://docs.rs/stumpalo/)
[![crates.io badge](https://img.shields.io/crates/v/stumpalo.svg?logo=rust)](https://crates.io/crates/stumpalo)
[![lobste.rs badge](https://img.shields.io/badge/lobste.rs-discuss-blue?logo=lobsters)](https://lobste.rs/s/vta6wp/faster_bump_allocator_for_rust)



A fast, zero-dependency, memory-efficient bump allocator with chunk reuse and
scoped stack support.

## Features

#### Fast allocations

The fast path contains as little as six instructions, and one conditional branch

#### Chunk reuse

`.clear()` resets the arena for reuse without freeing memory, allocated chunks are recycled on subsequent allocations.

#### Stack allocation support

`with_scope()` creates a temporary sub-arena whose
allocations only live as long as the closure.
This lets you use an arena as a heterogeneous stack allocator.

## Usage

### Basic allocation

```rust
use stumpalo::Arena;

let arena = Arena::new();
let x = arena.alloc(42u32);
let y = arena.alloc_with(|| 2 + 12);
```

### Slices and arrays

```rust
use stumpalo::Arena;
let arena = Arena::new();

// Copy a slice in
let nums = arena.alloc_slice_copy(&[1, 2, 3, 4, 5]);
assert_eq!(nums, &[1, 2, 3, 4, 5]);

// Allocate a fixed-size array with Default
let arr: &mut [u8] = arena.alloc_slice_fill_default(12);
assert_eq!(arr.len(), 12);

// Allocate a slice of length 12, filled by a function which takes the index
let arr: &mut [u8] = arena.alloc_slice_fill_with(12, |n| n as u8 + 1);
assert_eq!(arr[2], 3);

// Allocate a string
let s: &mut str = arena.alloc_str("hello world");
assert_eq!(s, "hello world");
```

### Scoped arenas

```rust
use stumpalo::Arena;
let mut arena = Arena::new();

// This is only necessary if you need to keep references alive from
// the outer scope, after the inner scope returns.
let arena = arena.as_arena_ref_mut();

let a = arena.alloc(1u32);

arena.with_scope(|scope| {
    let temporary = scope.alloc(2u32);
    // temporary lives only for this scope
});
// Scope returned, inner allocations are gone

// 'a' is still accessible
assert_eq!(*a, 1);
```

### Clear and reuse

```rust
use stumpalo::Arena;
let mut arena = Arena::new();
let x = arena.alloc(42u32);
arena.clear(); // x is now invalid, trying to use it will yield a compile-time error.

let y = arena.alloc(10u32); // reuses the same chunk
assert_eq!(*y, 10);
```

### Specialized allocation methods

If your slice has a length known at compile-time, use `alloc_sized_slice_copy` instead of `alloc_slice_copy`.

If you have a string literal, use `alloc_str_lit` instead of `alloc_str`.

These methods can sometimes use the extra compile-time information to reduce the size of the fast-path.

## Benchmarks

stumpalo includes a comparison benchmark against [bumpalo](https://crates.io/crates/bumpalo)
and [blink-alloc](https://crates.io/crates/blink-alloc).

Each library measurement runs in its own forked process to eliminate
heap-fragmentation interference.

### Running

```bash
cargo bench
```

Each cell shows the slowdown relative to the fastest library for that
operation (1.00x = fastest).

✅ <1.02x  🟢 <1.10x  🟡 <1.25x  🟠 <1.50x  🔴 <2.00x  🟥 2.00x+

### Results

<details>

<summary>Expand benchmark run</summary>

Benchmark machine: AMD Ryzen 3900x, Arch Linux, kernel 7.0.3-arch1-2

```txt
Num allocs: 100000
Warmup: 10
Samples: 60
Discarded samples: 6
operation                        stumpalo              blink-alloc           bumpalo
alloc_u8                         ✅  1.00x   128.8 µs  🔴  2.14x   275.0 µs  🟠  1.54x   197.8 µs
alloc_u16                        ✅  1.00x   111.0 µs  🔴  2.46x   272.6 µs  🟥  2.54x   281.5 µs
alloc_u32                        ✅  1.00x    74.8 µs  🟥  3.36x   251.0 µs  🟥  3.34x   250.0 µs
alloc_u64                        ✅  1.00x    75.1 µs  🟥  3.35x   251.5 µs  🟥  3.34x   250.8 µs
alloc_u128                       ✅  1.00x   215.5 µs  🟡  1.19x   256.6 µs  🟡  1.18x   254.4 µs
alloc_multiple_u8                ✅  1.00x  1032.5 µs  🔴  1.82x  1874.1 µs  🔴  1.85x  1907.0 µs
alloc_multiple_u16               ✅  1.00x   886.7 µs  🔴  2.30x  2043.5 µs  🔴  2.34x  2071.3 µs
alloc_multiple_u32               ✅  1.00x   654.4 µs  🟥  3.12x  2043.7 µs  🟥  3.14x  2056.1 µs
alloc_multiple_u64               ✅  1.00x   633.0 µs  🟥  3.23x  2044.3 µs  🟥  3.25x  2055.3 µs
alloc_multiple_u128              ✅  1.00x   758.9 µs  🟥  2.70x  2045.9 µs  🟥  2.61x  1978.3 µs
alloc_array_u8_8                 ✅  1.00x    99.4 µs  🔴  1.99x   197.7 µs  🔴  2.11x   210.0 µs
alloc_array_u8_32                ✅  1.00x   179.1 µs  🟢  1.15x   205.4 µs  🟡  1.20x   215.1 µs
alloc_array_u8_64                ✅  1.00x   135.2 µs  🟠  1.55x   209.0 µs  🟠  1.59x   214.9 µs
alloc_array_u8_128               ✅  1.00x   187.7 µs  🟡  1.30x   244.7 µs  🟠  1.50x   280.8 µs
alloc_slice_u8_8                 🟢  1.11x   248.0 µs  🟡  1.27x   283.2 µs  ✅  1.00x   222.5 µs
alloc_slice_u8_32                🟢  1.06x   274.9 µs  ✅  1.00x   259.6 µs  🟢  1.08x   279.4 µs
alloc_slice_u8_64                ✅  1.05x   278.7 µs  ✅  1.00x   266.7 µs  🟢  1.09x   289.6 µs
alloc_slice_u8_128               ✅  1.00x   333.3 µs  🟢  1.06x   353.0 µs  ✅  1.04x   347.4 µs
alloc_slice_u16_8                ✅  1.00x   229.4 µs  🟡  1.33x   304.3 µs  🟡  1.16x   265.5 µs
alloc_slice_u16_32               ✅  1.00x   302.4 µs  🟢  1.14x   345.3 µs  🟢  1.11x   334.2 µs
alloc_slice_u16_64               ✅  1.00x   358.4 µs  🟢  1.14x   408.4 µs  🟢  1.10x   394.5 µs
alloc_slice_u16_128              ✅  1.04x  3401.3 µs  ✅  1.00x  3270.6 µs  ✅  1.02x  3336.0 µs
alloc_slice_u32_8                ✅  1.00x   300.0 µs  🟢  1.14x   341.6 µs  🟢  1.09x   327.6 µs
alloc_slice_u32_32               ✅  1.00x   358.1 µs  🟢  1.14x   409.1 µs  🟢  1.10x   393.9 µs
alloc_slice_u32_64               ✅  1.05x  3459.3 µs  ✅  1.00x  3309.5 µs  🟢  1.06x  3509.9 µs
alloc_slice_u32_128              🟢  1.09x  7676.0 µs  ✅  1.00x  7027.1 µs  🟢  1.13x  7912.7 µs
alloc_slice_u64_8                ✅  1.00x   274.8 µs  🟡  1.25x   344.8 µs  🟢  1.11x   305.0 µs
alloc_slice_u64_32               ✅  1.04x  3422.2 µs  ✅  1.00x  3297.0 µs  ✅  1.02x  3351.6 µs
alloc_slice_u64_64               🟢  1.08x  7600.2 µs  ✅  1.00x  7036.4 µs  🟢  1.10x  7751.8 µs
alloc_slice_u64_128              🟢  1.07x 14956.8 µs  ✅  1.00x 14003.1 µs  🟢  1.08x 15073.6 µs
alloc_slice_u128_8               ✅  1.00x   357.5 µs  🟢  1.12x   401.9 µs  🟢  1.11x   395.1 µs
alloc_slice_u128_32              🟢  1.08x  7595.3 µs  ✅  1.00x  7032.0 µs  🟢  1.12x  7898.0 µs
alloc_slice_u128_64              🟢  1.07x 15015.9 µs  ✅  1.00x 14065.2 µs  🟢  1.08x 15194.2 µs
alloc_slice_u128_128             ✅  1.03x 29365.9 µs  ✅  1.00x 28623.2 µs  ✅  1.04x 29681.1 µs
alloc_sized_slice_u8_8           ✅  1.00x   157.6 µs  🟡  1.28x   201.6 µs  🟡  1.34x   211.1 µs
alloc_sized_slice_u8_32          ✅  1.00x   102.8 µs  🔴  1.91x   196.4 µs  🔴  2.24x   230.5 µs
alloc_sized_slice_u8_64          ✅  1.00x   146.1 µs  🟠  1.51x   221.3 µs  🟠  1.54x   224.3 µs
alloc_sized_slice_u8_128         ✅  1.00x   247.2 µs  🟢  1.10x   271.6 µs  🟢  1.14x   282.4 µs
alloc_sized_slice_u16_8          ✅  1.00x   110.1 µs  🔴  2.19x   240.7 µs  🔴  2.12x   233.3 µs
alloc_sized_slice_u16_32         ✅  1.00x   154.3 µs  🟠  1.60x   246.8 µs  🟠  1.53x   235.4 µs
alloc_sized_slice_u16_64         ✅  1.00x   255.7 µs  🟢  1.14x   291.8 µs  🟢  1.09x   279.1 µs
alloc_sized_slice_u16_128        ✅  1.00x  3278.2 µs  ✅  1.01x  3299.8 µs  ✅  1.01x  3305.7 µs
alloc_sized_slice_u32_8          ✅  1.00x   179.1 µs  🟠  1.41x   252.8 µs  🟠  1.37x   244.9 µs
alloc_sized_slice_u32_32         ✅  1.00x   261.1 µs  🟡  1.16x   301.7 µs  🟡  1.24x   323.0 µs
alloc_sized_slice_u32_64         ✅  1.02x  3405.4 µs  ✅  1.00x  3340.3 µs  🟢  1.11x  3695.3 µs
alloc_sized_slice_u32_128        ✅  1.05x  7576.7 µs  ✅  1.00x  7245.1 µs  🟢  1.08x  7831.9 µs
alloc_sized_slice_u64_8          ✅  1.00x   235.2 µs  🟢  1.07x   252.0 µs  🟢  1.10x   257.6 µs
alloc_sized_slice_u64_32         ✅  1.00x  3300.0 µs  ✅  1.02x  3360.5 µs  ✅  1.02x  3351.7 µs
alloc_sized_slice_u64_64         🟢  1.07x  7595.2 µs  ✅  1.00x  7080.6 µs  🟢  1.11x  7832.8 µs
alloc_sized_slice_u64_128        🟢  1.05x 14940.6 µs  ✅  1.00x 14174.8 µs  🟢  1.07x 15175.5 µs
alloc_sized_slice_u128_8         ✅  1.00x   220.1 µs  🟡  1.24x   271.9 µs  🟡  1.21x   265.5 µs
alloc_sized_slice_u128_32        🟢  1.07x  7576.7 µs  ✅  1.00x  7063.4 µs  🟢  1.11x  7819.6 µs
alloc_sized_slice_u128_64        🟢  1.07x 14981.6 µs  ✅  1.00x 14055.3 µs  🟢  1.08x 15198.5 µs
alloc_sized_slice_u128_128       ✅  1.02x 29366.2 µs  ✅  1.00x 28659.9 µs  ✅  1.03x 29414.6 µs
alloc_struct_13                  ✅  1.00x   161.3 µs  🟠  1.55x   250.1 µs  🟠  1.39x   224.4 µs
alloc_struct_24                  ✅  1.00x   108.3 µs  🔴  1.94x   209.6 µs  🔴  1.97x   213.6 µs
alloc_struct_26                  ✅  1.00x   155.7 µs  🟠  1.56x   242.4 µs  🟠  1.52x   236.2 µs
alloc_struct_30                  ✅  1.00x   159.1 µs  🟠  1.54x   245.8 µs  🟠  1.45x   230.1 µs
alloc_struct_32                  ✅  1.00x   151.5 µs  🟠  1.35x   204.9 µs  🟠  1.40x   211.7 µs
alloc_struct_64                  ✅  1.00x   145.3 µs  🟠  1.44x   208.7 µs  🟠  1.48x   215.1 µs
alloc_struct_96                  ✅  1.00x   188.0 µs  🟢  1.13x   211.6 µs  🟡  1.18x   222.6 µs
alloc_struct_128                 ✅  1.00x   209.1 µs  🟡  1.33x   278.4 µs  🟡  1.17x   245.2 µs
alloc_struct_192                 ✅  1.02x  1401.8 µs  ✅  1.00x  1371.7 µs  🟢  1.09x  1492.7 µs
alloc_struct_256                 ✅  1.00x  3324.6 µs  🟡  1.16x  3845.6 µs  ✅  1.01x  3368.2 µs
alloc_struct_512                 🟢  1.06x  7708.0 µs  ✅  1.00x  7269.0 µs  ✅  1.02x  7419.6 µs
alloc_struct_1k                  ✅  1.00x 14883.4 µs  🟢  1.05x 15633.8 µs  ✅  1.01x 15084.8 µs
alloc_struct_half_chunk_minus_1  ✅  1.00x   345.1 µs  ✅  1.04x   357.6 µs  🟢  1.11x   382.6 µs
alloc_struct_half_chunk          ✅  1.00x   217.0 µs  🟢  1.12x   243.7 µs  🟡  1.28x   276.7 µs
alloc_struct_half_chunk_plus_1   ✅  1.00x   353.7 µs  ✅  1.00x   354.1 µs  ✅  1.04x   366.3 µs
alloc_struct_one_chunk_minus_1   ✅  1.00x  2916.8 µs  ✅  1.02x  2975.2 µs  ✅  1.02x  2968.8 µs
alloc_struct_one_chunk           ✅  1.00x  2924.1 µs  ✅  1.01x  2953.9 µs  ✅  1.03x  3021.9 µs
alloc_struct_one_chunk_plus_1    ✅  1.00x  2949.6 µs  ✅  1.03x  3027.1 µs  ✅  1.02x  3016.5 µs
alloc_struct_two_chunks          ✅  1.00x  6645.9 µs  ✅  1.01x  6693.1 µs  ✅  1.02x  6749.7 µs
alloc_str_8                      🟢  1.11x   246.5 µs  ✅  1.05x   232.7 µs  ✅  1.00x   221.9 µs
alloc_str_16                     🟢  1.07x   237.9 µs  ✅  1.02x   226.7 µs  ✅  1.00x   223.3 µs
alloc_str_32                     ✅  1.04x   257.5 µs  ✅  1.00x   247.0 µs  🟢  1.07x   265.4 µs
alloc_str_40                     ✅  1.00x   275.4 µs  🟢  1.08x   296.9 µs  🟢  1.06x   293.0 µs
alloc_str_48                     ✅  1.00x   256.1 µs  ✅  1.03x   263.8 µs  🟢  1.06x   272.2 µs
alloc_str_64                     ✅  1.00x   267.1 µs  ✅  1.04x   277.1 µs  🟢  1.06x   283.8 µs
alloc_str_72                     ✅  1.04x   326.2 µs  ✅  1.00x   313.5 µs  🟢  1.07x   334.4 µs
alloc_str_80                     ✅  1.03x   328.7 µs  ✅  1.00x   318.9 µs  🟢  1.07x   341.1 µs
alloc_str_128                    ✅  1.00x   334.8 µs  🟢  1.11x   371.7 µs  🟢  1.08x   363.2 µs
alloc_slice_lit_u8_8             ✅  1.00x    99.6 µs  🔴  2.47x   246.5 µs  🔴  2.23x   222.1 µs
alloc_slice_lit_u8_32            ✅  1.00x   163.0 µs  🔴  1.83x   298.2 µs  🟠  1.71x   279.4 µs
alloc_slice_lit_u8_64            ✅  1.00x   204.3 µs  🟡  1.34x   274.1 µs  🟠  1.42x   289.5 µs
alloc_slice_lit_u8_128           ✅  1.00x   266.7 µs  🟡  1.31x   349.0 µs  🟡  1.31x   349.3 µs
alloc_str_lit_8                  ✅  1.00x   122.3 µs  🔴  2.02x   246.5 µs  🔴  1.82x   222.3 µs
alloc_str_lit_16                 ✅  1.00x   139.5 µs  🔴  1.78x   248.5 µs  🟠  1.60x   223.0 µs
alloc_str_lit_32                 ✅  1.00x   197.1 µs  🟠  1.51x   297.6 µs  🟠  1.42x   279.1 µs
alloc_str_lit_40                 ✅  1.00x   148.0 µs  🔴  1.76x   260.4 µs  🔴  1.93x   285.4 µs
alloc_str_lit_48                 ✅  1.00x   156.7 µs  🟠  1.74x   272.3 µs  🔴  1.82x   285.9 µs
alloc_str_lit_64                 ✅  1.00x   171.3 µs  🔴  1.75x   300.7 µs  🟠  1.69x   289.4 µs
alloc_str_lit_72                 ✅  1.00x   208.5 µs  🟠  1.53x   318.5 µs  🟠  1.61x   336.3 µs
alloc_str_lit_80                 ✅  1.00x   204.7 µs  🟠  1.54x   315.1 µs  🟠  1.63x   334.1 µs
alloc_str_lit_128                ✅  1.00x   258.4 µs  🟠  1.36x   350.8 µs  🟠  1.35x   349.8 µs
clear                            ✅  1.00x   241.6 µs  ✅  1.04x   251.9 µs  ✅  1.04x   251.0 µs
clear_and_reuse                  ✅  1.00x    75.1 µs  🟥  3.35x   251.6 µs  🟥  3.35x   251.8 µs
```

</details>

## miri

This library uses `unsafe` internally unapologetically, but is miri-clean.
Run the integration test suite under miri to check for undefined behavior.

```bash
cargo +nightly miri test --test integration_test
```
