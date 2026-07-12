// Comparison benchmark: stumpalo vs bumpalo vs blink-alloc.
//
// No external harness (harness = false in Cargo.toml). Measures raw allocation
// throughput using std::time::Instant and prints a relative-performance table
// to stdout.
//
use libc::{close, fork, pipe, read, waitpid, write};
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use std::ffi::c_void;
use std::hint::black_box;
use std::time::{Duration, Instant};

const NUM_ALLOCS: usize = 100_000;
const WARMUP: usize = 10;
const SAMPLES: usize = 60;
const OUTLIERS: usize = 6;

// --- Static data for alloc_slice_lit_copy / alloc_str_lit benchmarks ---

const SLICE_LIT_U8_8: [u8; 8] = [0u8; 8];
const SLICE_LIT_U8_32: [u8; 32] = [0u8; 32];
const SLICE_LIT_U8_64: [u8; 64] = [0u8; 64];
const SLICE_LIT_U8_128: [u8; 128] = [0u8; 128];

// NOTE: these use &[u8; N] rather than &str so that the const generic N
// carries compile-time size information through to alloc_slice_lit_copy.
// In practice, users call alloc_str_lit("literal") directly and the
// compiler sees the string length at compile time without this workaround.
const STR_LIT_8: &[u8; 8] = b"aaaaaaaa";
const STR_LIT_16: &[u8; 16] = b"aaaaaaaaaaaaaaaa";
const STR_LIT_32: &[u8; 32] = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const STR_LIT_40: &[u8; 40] = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const STR_LIT_48: &[u8; 48] = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const STR_LIT_64: &[u8; 64] = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const STR_LIT_72: &[u8; 72] =
    b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const STR_LIT_80: &[u8; 80] =
    b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const STR_LIT_128: &[u8; 128] = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Run `f` in a forked child process and return the resulting `Duration`.
/// This ensures a pristine heap for each measurement, eliminating heap
/// fragmentation interference between benchmarks.
fn fork_measure(f: impl FnOnce() -> Duration) -> Duration {
    let mut fds = [0i32; 2];
    unsafe {
        if pipe(fds.as_mut_ptr()) != 0 {
            panic!("pipe failed");
        }
    }

    match unsafe { fork() } {
        -1 => panic!("fork failed"),
        0 => {
            // Child: run the measurement, send duration, exit.
            unsafe { close(fds[0]) };
            let ns = f().as_nanos() as u64;
            unsafe {
                write(
                    fds[1],
                    &ns as *const u64 as *const c_void,
                    std::mem::size_of::<u64>(),
                );
                close(fds[1]);
            }
            std::process::exit(0);
        }
        pid => {
            // Parent: wait for child and read the duration.
            unsafe { close(fds[1]) };
            let mut status = 0;
            unsafe { waitpid(pid, &mut status, 0) };
            let mut ns: u64 = 0;
            unsafe {
                read(
                    fds[0],
                    &mut ns as *mut u64 as *mut c_void,
                    std::mem::size_of::<u64>(),
                );
                close(fds[0]);
            }
            Duration::from_nanos(ns)
        }
    }
}

fn main() {
    println!("Num allocs: {}", NUM_ALLOCS);
    println!("Warmup: {}", WARMUP);
    println!("Samples: {}", SAMPLES);
    println!("Discarded samples: {}", OUTLIERS);

    // Warm up the CPU frequency governor + cache before any measurements.
    // Modern CPUs take several hundred ms to ramp up turbo.
    warmup_system();

    let mut results = Vec::new();

    // --- alloc scalar ---
    results.push(bench_alloc::<u8>("alloc_u8"));
    results.push(bench_alloc::<u16>("alloc_u16"));
    results.push(bench_alloc::<u32>("alloc_u32"));
    results.push(bench_alloc::<u64>("alloc_u64"));
    results.push(bench_alloc::<u128>("alloc_u128"));

    // --- alloc multiple scalars ---
    results.push(bench_alloc_multiple::<u8>("alloc_multiple_u8"));
    results.push(bench_alloc_multiple::<u16>("alloc_multiple_u16"));
    results.push(bench_alloc_multiple::<u32>("alloc_multiple_u32"));
    results.push(bench_alloc_multiple::<u64>("alloc_multiple_u64"));
    results.push(bench_alloc_multiple::<u128>("alloc_multiple_u128"));

    // --- alloc array ---
    // Use alloc_with to avoid the intermediate stack-copy artifact:
    //   a.alloc([T::default(); N])                 ←  creates + zeros stack array, then copies in
    //   a.alloc_with(|| [T::default(); N])          ←  closure is called inside, no intermediate copy
    results.push(bench_alloc_array::<u8, 8>("alloc_array_u8_8"));
    results.push(bench_alloc_array::<u8, 32>("alloc_array_u8_32"));
    results.push(bench_alloc_array::<u8, 64>("alloc_array_u8_64"));
    results.push(bench_alloc_array::<u8, 128>("alloc_array_u8_128"));

    // --- alloc_slice_copy ---
    results.push(bench_alloc_slice::<u8, 8>("alloc_slice_u8_8"));
    results.push(bench_alloc_slice::<u8, 32>("alloc_slice_u8_32"));
    results.push(bench_alloc_slice::<u8, 64>("alloc_slice_u8_64"));
    results.push(bench_alloc_slice::<u8, 128>("alloc_slice_u8_128"));

    results.push(bench_alloc_slice::<u16, 8>("alloc_slice_u16_8"));
    results.push(bench_alloc_slice::<u16, 32>("alloc_slice_u16_32"));
    results.push(bench_alloc_slice::<u16, 64>("alloc_slice_u16_64"));
    results.push(bench_alloc_slice::<u16, 128>("alloc_slice_u16_128"));

    results.push(bench_alloc_slice::<u32, 8>("alloc_slice_u32_8"));
    results.push(bench_alloc_slice::<u32, 32>("alloc_slice_u32_32"));
    results.push(bench_alloc_slice::<u32, 64>("alloc_slice_u32_64"));
    results.push(bench_alloc_slice::<u32, 128>("alloc_slice_u32_128"));

    results.push(bench_alloc_slice::<u64, 8>("alloc_slice_u64_8"));
    results.push(bench_alloc_slice::<u64, 32>("alloc_slice_u64_32"));
    results.push(bench_alloc_slice::<u64, 64>("alloc_slice_u64_64"));
    results.push(bench_alloc_slice::<u64, 128>("alloc_slice_u64_128"));

    results.push(bench_alloc_slice::<u128, 8>("alloc_slice_u128_8"));
    results.push(bench_alloc_slice::<u128, 32>("alloc_slice_u128_32"));
    results.push(bench_alloc_slice::<u128, 64>("alloc_slice_u128_64"));
    results.push(bench_alloc_slice::<u128, 128>("alloc_slice_u128_128"));

    // --- alloc_sized_slice_copy ---
    results.push(bench_alloc_sized_slice::<u8, 8>("alloc_sized_slice_u8_8"));
    results.push(bench_alloc_sized_slice::<u8, 32>("alloc_sized_slice_u8_32"));
    results.push(bench_alloc_sized_slice::<u8, 64>("alloc_sized_slice_u8_64"));
    results.push(bench_alloc_sized_slice::<u8, 128>(
        "alloc_sized_slice_u8_128",
    ));

    results.push(bench_alloc_sized_slice::<u16, 8>("alloc_sized_slice_u16_8"));
    results.push(bench_alloc_sized_slice::<u16, 32>(
        "alloc_sized_slice_u16_32",
    ));
    results.push(bench_alloc_sized_slice::<u16, 64>(
        "alloc_sized_slice_u16_64",
    ));
    results.push(bench_alloc_sized_slice::<u16, 128>(
        "alloc_sized_slice_u16_128",
    ));

    results.push(bench_alloc_sized_slice::<u32, 8>("alloc_sized_slice_u32_8"));
    results.push(bench_alloc_sized_slice::<u32, 32>(
        "alloc_sized_slice_u32_32",
    ));
    results.push(bench_alloc_sized_slice::<u32, 64>(
        "alloc_sized_slice_u32_64",
    ));
    results.push(bench_alloc_sized_slice::<u32, 128>(
        "alloc_sized_slice_u32_128",
    ));

    results.push(bench_alloc_sized_slice::<u64, 8>("alloc_sized_slice_u64_8"));
    results.push(bench_alloc_sized_slice::<u64, 32>(
        "alloc_sized_slice_u64_32",
    ));
    results.push(bench_alloc_sized_slice::<u64, 64>(
        "alloc_sized_slice_u64_64",
    ));
    results.push(bench_alloc_sized_slice::<u64, 128>(
        "alloc_sized_slice_u64_128",
    ));

    results.push(bench_alloc_sized_slice::<u128, 8>(
        "alloc_sized_slice_u128_8",
    ));
    results.push(bench_alloc_sized_slice::<u128, 32>(
        "alloc_sized_slice_u128_32",
    ));
    results.push(bench_alloc_sized_slice::<u128, 64>(
        "alloc_sized_slice_u128_64",
    ));
    results.push(bench_alloc_sized_slice::<u128, 128>(
        "alloc_sized_slice_u128_128",
    ));

    // --- alloc big struct ---
    // Use round sizes and sizes relative to INITIAL_CHUNK_CAPACITY to exercise
    // the various allocation paths (normal chunk, > half-chunk, full chunk, etc.).
    results.push(bench_alloc_struct::<13>("alloc_struct_13"));
    results.push(bench_alloc_struct::<24>("alloc_struct_24"));
    results.push(bench_alloc_struct::<26>("alloc_struct_26"));
    results.push(bench_alloc_struct::<30>("alloc_struct_30"));
    results.push(bench_alloc_struct::<32>("alloc_struct_32"));
    results.push(bench_alloc_struct::<64>("alloc_struct_64"));
    results.push(bench_alloc_struct::<96>("alloc_struct_96"));
    results.push(bench_alloc_struct::<128>("alloc_struct_128"));
    results.push(bench_alloc_struct::<192>("alloc_struct_192"));
    results.push(bench_alloc_struct::<256>("alloc_struct_256"));
    results.push(bench_alloc_struct::<512>("alloc_struct_512"));
    results.push(bench_alloc_struct::<1024>("alloc_struct_1k"));
    // Sizes relative to INITIAL_CHUNK_CAPACITY.
    results.push(bench_alloc_struct::<
        { stumpalo::INITIAL_CHUNK_CAPACITY / 2 - 1 },
    >("alloc_struct_half_chunk_minus_1"));
    results.push(
        bench_alloc_struct::<{ stumpalo::INITIAL_CHUNK_CAPACITY / 2 }>("alloc_struct_half_chunk"),
    );
    results.push(bench_alloc_struct::<
        { stumpalo::INITIAL_CHUNK_CAPACITY / 2 + 1 },
    >("alloc_struct_half_chunk_plus_1"));
    results.push(
        bench_alloc_struct::<{ stumpalo::INITIAL_CHUNK_CAPACITY - 1 }>(
            "alloc_struct_one_chunk_minus_1",
        ),
    );
    results.push(bench_alloc_struct::<{ stumpalo::INITIAL_CHUNK_CAPACITY }>(
        "alloc_struct_one_chunk",
    ));
    results.push(
        bench_alloc_struct::<{ stumpalo::INITIAL_CHUNK_CAPACITY + 1 }>(
            "alloc_struct_one_chunk_plus_1",
        ),
    );
    results.push(
        bench_alloc_struct::<{ stumpalo::INITIAL_CHUNK_CAPACITY * 2 }>("alloc_struct_two_chunks"),
    );

    // --- alloc_str ---
    results.push(bench_alloc_str("alloc_str_8", STR_LIT_8));
    results.push(bench_alloc_str("alloc_str_16", STR_LIT_16));
    results.push(bench_alloc_str("alloc_str_32", STR_LIT_32));
    results.push(bench_alloc_str("alloc_str_40", STR_LIT_40));
    results.push(bench_alloc_str("alloc_str_48", STR_LIT_48));
    results.push(bench_alloc_str("alloc_str_64", STR_LIT_64));
    results.push(bench_alloc_str("alloc_str_72", STR_LIT_72));
    results.push(bench_alloc_str("alloc_str_80", STR_LIT_80));
    results.push(bench_alloc_str("alloc_str_128", STR_LIT_128));

    // --- alloc_slice_lit_copy ---
    results.push(bench_alloc_slice_lit_u8::<8>(
        "alloc_slice_lit_u8_8",
        &SLICE_LIT_U8_8,
    ));
    results.push(bench_alloc_slice_lit_u8::<32>(
        "alloc_slice_lit_u8_32",
        &SLICE_LIT_U8_32,
    ));
    results.push(bench_alloc_slice_lit_u8::<64>(
        "alloc_slice_lit_u8_64",
        &SLICE_LIT_U8_64,
    ));
    results.push(bench_alloc_slice_lit_u8::<128>(
        "alloc_slice_lit_u8_128",
        &SLICE_LIT_U8_128,
    ));

    // --- alloc_str_lit ---
    results.push(bench_alloc_str_lit::<8>("alloc_str_lit_8", STR_LIT_8));
    results.push(bench_alloc_str_lit::<16>("alloc_str_lit_16", STR_LIT_16));
    results.push(bench_alloc_str_lit::<32>("alloc_str_lit_32", STR_LIT_32));
    results.push(bench_alloc_str_lit::<40>("alloc_str_lit_40", STR_LIT_40));
    results.push(bench_alloc_str_lit::<48>("alloc_str_lit_48", STR_LIT_48));
    results.push(bench_alloc_str_lit::<64>("alloc_str_lit_64", STR_LIT_64));
    results.push(bench_alloc_str_lit::<72>("alloc_str_lit_72", STR_LIT_72));
    results.push(bench_alloc_str_lit::<80>("alloc_str_lit_80", STR_LIT_80));
    results.push(bench_alloc_str_lit::<128>("alloc_str_lit_128", STR_LIT_128));

    // --- clear ---
    results.push(bench_clear());

    // --- clear_and_reuse ---
    results.push(bench_clear_and_reuse());

    // --- output ---
    let tsv = tsv_table(&results);
    let formatted = pipe_through_column(&tsv);
    print!("{formatted}");
}

// ---------------------------------------------------------------------------
// System warmup — drive the CPU to max turbo before measuring
// ---------------------------------------------------------------------------

/// Run a tight loop for ~1s to stabilise CPU frequency governor + cache state.
fn warmup_system() {
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut x: u64 = 0;
    while Instant::now() < deadline {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        black_box(x);
    }
}

// ---------------------------------------------------------------------------
// Measurement helpers
// ---------------------------------------------------------------------------

/// Run `f` once and return the elapsed wall time.
fn once<F: FnOnce()>(f: F) -> Duration {
    let start = Instant::now();
    f();
    start.elapsed()
}

/// Run warmup iterations, then collect `n` measured samples, returning a sorted copy.
fn sample<F: FnMut()>(n: usize, mut f: F) -> Vec<Duration> {
    for _ in 0..WARMUP {
        f();
    }
    let mut out: Vec<Duration> = (0..n).map(|_| once(&mut f)).collect();
    out.sort();
    out
}

/// Return the mean of the pre-sorted slice, dropping the worst N outliers.
fn mean_without_outliers(s: &[Duration]) -> Duration {
    let keep = s.len().saturating_sub(OUTLIERS);
    if keep == 0 {
        return Duration::ZERO;
    }
    let sum_ns: u128 = s[..keep].iter().map(|d| d.as_nanos() as u128).sum();
    Duration::from_nanos((sum_ns / keep as u128) as u64)
}

// ---------------------------------------------------------------------------
// Per-library measurement functions
// ---------------------------------------------------------------------------

fn measure_stump_alloc<T: Default>() -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let a = stumpalo::Arena::new();
        for _ in 0..NUM_ALLOCS {
            black_box(a.alloc(T::default()));
        }
    }))
}

fn measure_bump_alloc<T: Default>() -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let b = bumpalo::Bump::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.alloc(T::default()));
        }
    }))
}

fn measure_blink_alloc<T: Default + 'static>() -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let b = blink_alloc::Blink::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.put(T::default()));
        }
    }))
}

fn measure_stump_alloc_multiple<T: Default>() -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let a = stumpalo::Arena::new();
        for _ in 0..NUM_ALLOCS {
            black_box(a.alloc(T::default()));
            black_box(a.alloc(T::default()));
            black_box(a.alloc(T::default()));
            black_box(a.alloc(T::default()));
            black_box(a.alloc(T::default()));
            black_box(a.alloc(T::default()));
            black_box(a.alloc(T::default()));
            black_box(a.alloc(T::default()));
        }
    }))
}

fn measure_bump_alloc_multiple<T: Default>() -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let b = bumpalo::Bump::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.alloc(T::default()));
            black_box(b.alloc(T::default()));
            black_box(b.alloc(T::default()));
            black_box(b.alloc(T::default()));
            black_box(b.alloc(T::default()));
            black_box(b.alloc(T::default()));
            black_box(b.alloc(T::default()));
            black_box(b.alloc(T::default()));
        }
    }))
}

fn measure_blink_alloc_multiple<T: Default + 'static>() -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let b = blink_alloc::Blink::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.put(T::default()));
            black_box(b.put(T::default()));
            black_box(b.put(T::default()));
            black_box(b.put(T::default()));
            black_box(b.put(T::default()));
            black_box(b.put(T::default()));
            black_box(b.put(T::default()));
            black_box(b.put(T::default()));
        }
    }))
}

/// Measure array alloc using `alloc_with` to avoid the intermediate stack-copy
/// artifact: `a.alloc_with(|| [T::default(); N])` creates the array inside
/// the allocator, avoiding a separate stack zero + copy.
fn measure_stump_alloc_array<T: Default + Copy, const N: usize>() -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let a = stumpalo::Arena::new();
        for _ in 0..NUM_ALLOCS {
            black_box(a.alloc_sized_slice_copy(&[T::default(); N]));
        }
    }))
}

fn measure_bump_alloc_array<T: Default + Copy, const N: usize>() -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let b = bumpalo::Bump::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.alloc_with(|| [T::default(); N]));
        }
    }))
}

fn measure_blink_alloc_array<T: Default + Copy + 'static, const N: usize>() -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let b = blink_alloc::Blink::new();
        for _ in 0..NUM_ALLOCS {
            // blink-alloc doesn't have alloc_with; put(T) takes ownership.
            // We create the value outside, like the original bench.
            black_box(b.put([T::default(); N]));
        }
    }))
}

fn measure_stump_alloc_slice<T: Copy + Default, const N: usize>() -> Duration {
    let data: &[T] = &[T::default(); N];
    mean_without_outliers(&sample(SAMPLES, || {
        let a = stumpalo::Arena::new();
        for _ in 0..NUM_ALLOCS {
            black_box(a.alloc_slice_copy(data));
        }
    }))
}

fn measure_bump_alloc_slice<T: Copy + Default, const N: usize>() -> Duration {
    let data: &[T] = &[T::default(); N];
    mean_without_outliers(&sample(SAMPLES, || {
        let b = bumpalo::Bump::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.alloc_slice_copy(data));
        }
    }))
}

fn measure_blink_alloc_slice<T: Copy + Default, const N: usize>() -> Duration {
    let data: &[T] = &[T::default(); N];
    mean_without_outliers(&sample(SAMPLES, || {
        let b = blink_alloc::Blink::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.copy_slice(data));
        }
    }))
}

fn measure_stump_alloc_sized_slice<T: Copy + Default, const N: usize>() -> Duration {
    let data: &[T; N] = &[T::default(); N];
    mean_without_outliers(&sample(SAMPLES, || {
        let a = stumpalo::Arena::new();
        for _ in 0..NUM_ALLOCS {
            black_box(a.alloc_sized_slice_copy(data));
        }
    }))
}

fn measure_bump_alloc_sized_slice<T: Copy + Default, const N: usize>() -> Duration {
    let data: &[T; N] = &[T::default(); N];
    mean_without_outliers(&sample(SAMPLES, || {
        let b = bumpalo::Bump::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.alloc_slice_copy(data));
        }
    }))
}

fn measure_blink_alloc_sized_slice<T: Copy + Default, const N: usize>() -> Duration {
    let data: &[T; N] = &[T::default(); N];
    mean_without_outliers(&sample(SAMPLES, || {
        let b = blink_alloc::Blink::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.copy_slice(data));
        }
    }))
}

fn measure_stump_alloc_str(s: &str) -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let a = stumpalo::Arena::new();
        for _ in 0..NUM_ALLOCS {
            black_box(a.alloc_str(s));
        }
    }))
}

fn measure_bump_alloc_str(s: &str) -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let b = bumpalo::Bump::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.alloc_str(s));
        }
    }))
}

fn measure_blink_alloc_str(s: &str) -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let b = blink_alloc::Blink::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.copy_str(s));
        }
    }))
}

// --- alloc_slice_lit_copy (stumpalo only — no direct equivalents in other libs) ---

fn measure_stump_alloc_slice_lit<const N: usize>(data: &'static [u8; N]) -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let a = stumpalo::Arena::new();
        for _ in 0..NUM_ALLOCS {
            black_box(a.alloc_slice_lit_copy(data));
        }
    }))
}

// --- alloc_str_lit (stumpalo only — use alloc_str/copy_str as comparison baseline) ---

// Workaround: manually inline alloc_str_lit's body so that the compile-time
// size N propagates into alloc_slice_lit_copy. In real code, calling
// alloc_str_lit("literal") directly lets the compiler see the string length;
// this is only needed because we pass data through a generic function boundary.
fn measure_stump_alloc_str_lit<const N: usize>(data: &'static [u8; N]) -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let a = stumpalo::Arena::new();
        for _ in 0..NUM_ALLOCS {
            let slice = a.alloc_slice_lit_copy(data);
            // Safety: data is ASCII, so from_utf8_unchecked_mut is sound.
            // This mirrors what alloc_str_lit does internally.
            black_box(unsafe { core::str::from_utf8_unchecked_mut(slice) });
        }
    }))
}

fn measure_bump_alloc_str_lit(n: usize) -> Duration {
    let buf = vec![b'a'; n];
    let s: &str = std::str::from_utf8(&buf).unwrap();
    mean_without_outliers(&sample(SAMPLES, || {
        let b = bumpalo::Bump::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.alloc_str(s));
        }
    }))
}

fn measure_blink_alloc_str_lit(n: usize) -> Duration {
    let buf = vec![b'a'; n];
    let s: &str = std::str::from_utf8(&buf).unwrap();
    mean_without_outliers(&sample(SAMPLES, || {
        let b = blink_alloc::Blink::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.copy_str(s));
        }
    }))
}

// --- alloc big struct (sized via const generic, covers large-allocation paths) ---

fn measure_stump_alloc_big<const N: usize>() -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let a = stumpalo::Arena::new();
        for _ in 0..NUM_ALLOCS {
            black_box(a.alloc_with(|| [0u8; N]));
        }
    }))
}

fn measure_bump_alloc_big<const N: usize>() -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let b = bumpalo::Bump::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.alloc_with(|| [0u8; N]));
        }
    }))
}

fn measure_blink_alloc_big<const N: usize>() -> Duration {
    mean_without_outliers(&sample(SAMPLES, || {
        let b = blink_alloc::Blink::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.put([0u8; N]));
        }
    }))
}

fn bench_alloc_struct<const N: usize>(name: &'static str) -> BenchResult {
    BenchResult {
        name,
        stumpalo: fork_measure(|| measure_stump_alloc_big::<N>()),
        bumpalo: fork_measure(|| measure_bump_alloc_big::<N>()),
        blink: fork_measure(|| measure_blink_alloc_big::<N>()),
    }
}

// ---------------------------------------------------------------------------
// Bench functions returning BenchResult
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct BenchResult {
    name: &'static str,
    stumpalo: Duration,
    bumpalo: Duration,
    blink: Duration,
}

fn bench_alloc<T: Default + 'static>(name: &'static str) -> BenchResult {
    BenchResult {
        name,
        stumpalo: fork_measure(|| measure_stump_alloc::<T>()),
        bumpalo: fork_measure(|| measure_bump_alloc::<T>()),
        blink: fork_measure(|| measure_blink_alloc::<T>()),
    }
}

fn bench_alloc_multiple<T: Default + 'static>(name: &'static str) -> BenchResult {
    BenchResult {
        name,
        stumpalo: fork_measure(|| measure_stump_alloc_multiple::<T>()),
        bumpalo: fork_measure(|| measure_bump_alloc_multiple::<T>()),
        blink: fork_measure(|| measure_blink_alloc_multiple::<T>()),
    }
}

fn bench_alloc_array<T: Default + Copy + 'static, const N: usize>(
    name: &'static str,
) -> BenchResult {
    BenchResult {
        name,
        stumpalo: fork_measure(|| measure_stump_alloc_array::<T, N>()),
        bumpalo: fork_measure(|| measure_bump_alloc_array::<T, N>()),
        blink: fork_measure(|| measure_blink_alloc_array::<T, N>()),
    }
}

fn bench_alloc_slice<T: Copy + Default, const N: usize>(name: &'static str) -> BenchResult {
    BenchResult {
        name,
        stumpalo: fork_measure(|| measure_stump_alloc_slice::<T, N>()),
        bumpalo: fork_measure(|| measure_bump_alloc_slice::<T, N>()),
        blink: fork_measure(|| measure_blink_alloc_slice::<T, N>()),
    }
}

fn bench_alloc_sized_slice<T: Copy + Default, const N: usize>(name: &'static str) -> BenchResult {
    BenchResult {
        name,
        stumpalo: fork_measure(|| measure_stump_alloc_sized_slice::<T, N>()),
        bumpalo: fork_measure(|| measure_bump_alloc_sized_slice::<T, N>()),
        blink: fork_measure(|| measure_blink_alloc_sized_slice::<T, N>()),
    }
}

fn bench_alloc_str(name: &'static str, s: &[u8]) -> BenchResult {
    let s = unsafe { std::str::from_utf8_unchecked(s) };
    BenchResult {
        name,
        stumpalo: fork_measure(|| measure_stump_alloc_str(s)),
        bumpalo: fork_measure(|| measure_bump_alloc_str(s)),
        blink: fork_measure(|| measure_blink_alloc_str(s)),
    }
}

fn bench_alloc_slice_lit_u8<const N: usize>(
    name: &'static str,
    data: &'static [u8; N],
) -> BenchResult {
    // blink-alloc and bumpalo don't have slice_lit equivalents —
    // use alloc_slice_copy / copy_slice as comparison baselines.
    BenchResult {
        name,
        stumpalo: fork_measure(|| measure_stump_alloc_slice_lit::<N>(data)),
        bumpalo: fork_measure(|| measure_bump_alloc_slice::<u8, N>()),
        blink: fork_measure(|| measure_blink_alloc_slice::<u8, N>()),
    }
}

fn bench_alloc_str_lit<const N: usize>(name: &'static str, data: &'static [u8; N]) -> BenchResult {
    // bumpalo and blink-alloc don't have str_lit equivalents —
    // use alloc_str / copy_str as comparison baselines.
    BenchResult {
        name,
        stumpalo: fork_measure(|| measure_stump_alloc_str_lit::<N>(data)),
        bumpalo: fork_measure(|| measure_bump_alloc_str_lit(N)),
        blink: fork_measure(|| measure_blink_alloc_str_lit(N)),
    }
}

fn bench_clear() -> BenchResult {
    fn prepare_bump() -> bumpalo::Bump {
        let b = bumpalo::Bump::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.alloc(0u64));
        }
        b
    }
    fn prepare_blink() -> blink_alloc::Blink {
        let b = blink_alloc::Blink::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.put(0u64));
        }
        b
    }

    BenchResult {
        name: "clear",
        stumpalo: fork_measure(|| {
            mean_without_outliers(&sample(SAMPLES, || {
                let mut a = stumpalo::Arena::new();
                for _ in 0..NUM_ALLOCS {
                    black_box(a.alloc(0u64));
                }
                a.clear();
                black_box(a.chunk_capacity());
            }))
        }),
        bumpalo: fork_measure(|| {
            mean_without_outliers(&sample(SAMPLES, || {
                let mut b = prepare_bump();
                b.reset();
                black_box(());
            }))
        }),
        blink: fork_measure(|| {
            mean_without_outliers(&sample(SAMPLES, || {
                let mut b = prepare_blink();
                b.reset();
                black_box(());
            }))
        }),
    }
}

fn bench_clear_and_reuse() -> BenchResult {
    fn prepare_bump() -> bumpalo::Bump {
        let b = bumpalo::Bump::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.alloc(0u64));
        }
        b
    }
    fn prepare_blink() -> blink_alloc::Blink {
        let b = blink_alloc::Blink::new();
        for _ in 0..NUM_ALLOCS {
            black_box(b.put(0u64));
        }
        b
    }

    BenchResult {
        name: "clear_and_reuse",
        stumpalo: fork_measure(|| {
            mean_without_outliers(&sample(SAMPLES, || {
                let mut a = stumpalo::Arena::new();
                for _ in 0..NUM_ALLOCS {
                    black_box(a.alloc(0u64));
                }
                a.clear();
                black_box(a.alloc(0u64));
            }))
        }),
        bumpalo: fork_measure(|| {
            mean_without_outliers(&sample(SAMPLES, || {
                let mut b = prepare_bump();
                b.reset();
                black_box(b.alloc(0u64));
            }))
        }),
        blink: fork_measure(|| {
            mean_without_outliers(&sample(SAMPLES, || {
                let mut b = prepare_blink();
                b.reset();
                black_box(b.put(0u64));
            }))
        }),
    }
}

// ---------------------------------------------------------------------------
// TSV table generation
// ---------------------------------------------------------------------------

fn tsv_table(results: &[BenchResult]) -> String {
    let mut tsv = String::new();
    tsv.push_str("operation\tstumpalo\tblink-alloc\tbumpalo\n");
    for r in results {
        let st = fmt_cell(r.stumpalo, r.stumpalo, r.blink, r.bumpalo);
        let bl = fmt_cell(r.blink, r.stumpalo, r.blink, r.bumpalo);
        let bu = fmt_cell(r.bumpalo, r.stumpalo, r.blink, r.bumpalo);
        tsv.push_str(&format!("{}\t{}\t{}\t{}\n", r.name, st, bl, bu));
    }
    tsv
}

/// Format a single cell: `<emoji> <ratio>x  <duration>`.
/// Ratio is relative to the fastest of the three (self / fastest).
fn fmt_cell(d: Duration, stumpalo: Duration, blink: Duration, bumpalo: Duration) -> String {
    let fastest = stumpalo.min(blink).min(bumpalo);
    let ratio = d.as_nanos() as f64 / fastest.as_nanos() as f64;
    let emoji = ratio_emoji(ratio);
    format!(
        "{} {:>5.2}x {:>7.1} µs",
        emoji,
        ratio,
        d.as_nanos() as f64 / 1000.0
    )
}

fn ratio_emoji(ratio: f64) -> &'static str {
    if ratio <= 1.05 {
        "✅" // winner / within noise
    } else if ratio <= 1.15 {
        "🟢" // slight lead
    } else if ratio <= 1.35 {
        "🟡" // close
    } else if ratio <= 1.75 {
        "🟠" // noticeable
    } else if ratio <= 2.5 {
        "🔴" // substantial
    } else {
        "🟥" // blowout
    }
}

// ---------------------------------------------------------------------------
// column(1) formatting — pipe TSV through Unix `column -t -s $'\t'`
// ---------------------------------------------------------------------------

/// Pipe plain TSV through `column -t -s $'\t'` for aligned output.
/// Falls back to raw TSV if column(1) is not available.
fn pipe_through_column(tsv: &str) -> String {
    use std::process::{Command, Stdio};
    let mut child = match Command::new("column")
        .args(["-t", "-s", "\t"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .env("LC_CTYPE", "C.utf8")
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return tsv.to_owned(),
    };
    use std::io::Write;
    let _ = child.stdin.take().unwrap().write_all(tsv.as_bytes());
    match child.wait_with_output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
        Err(_) => tsv.to_owned(),
    }
}
