use libc::memset;
use std::{alloc::Layout, ffi::c_void};
use stumpalo::{Arena, INITIAL_CHUNK_CAPACITY};

// --- Helpers ---

// A type with a known, unusual alignment for alignment tests.
#[repr(align(64))]
#[allow(dead_code)]
struct Align64(u8);

// A large type whose Layout::new reports a big size, for triggering
// the large-allocation path.
#[repr(C)]
struct BigAlloc<const N: usize>([u8; N]);

// --- Construction & sizing ---

#[test]
fn arena_size_is_32_bytes() {
    assert_eq!(std::mem::size_of::<Arena>(), 32);
}

#[test]
fn new_creates_valid_arena() {
    let arena = Arena::new();
    // A fresh arena should have no fresh chunks
    assert!(!arena.has_fresh_chunks());
    // Lazy allocation: no chunk until first use
    assert_eq!(arena.chunk_capacity(), 0);
    // But allocation still works
    let x = arena.alloc(42u32);
    assert_eq!(*x, 42);
    assert!(arena.chunk_capacity() > 0);
}

#[test]
fn allocates_default_chunk_size() {
    let arena = Arena::new();
    arena.alloc(1u32);
    assert_eq!(
        arena.chunk_capacity(),
        INITIAL_CHUNK_CAPACITY - std::mem::size_of::<u32>()
    );
}

// --- Basic allocation ---

#[test]
fn alloc_bool() {
    let arena = Arena::new();
    let a = arena.alloc(true);
    let b = arena.alloc(false);
    assert_eq!(*a, true);
    assert_eq!(*b, false);
    *b = true;
    assert_eq!(*b, true);
}

#[test]
fn alloc_u8() {
    let arena = Arena::new();
    let x = arena.alloc(42u8);
    assert_eq!(*x, 42);
    *x = 255;
    assert_eq!(*x, 255);
}

#[test]
fn alloc_i32() {
    let arena = Arena::new();
    let a = arena.alloc(-1i32);
    let b = arena.alloc(0i32);
    let c = arena.alloc(i32::MAX);
    let d = arena.alloc(i32::MIN);
    assert_eq!(*a, -1);
    assert_eq!(*b, 0);
    assert_eq!(*c, i32::MAX);
    assert_eq!(*d, i32::MIN);
}

#[test]
fn alloc_f64() {
    let arena = Arena::new();
    let pi = arena.alloc(std::f64::consts::PI);
    let e = arena.alloc(std::f64::consts::E);
    assert!((*pi - std::f64::consts::PI).abs() < f64::EPSILON);
    assert!((*e - std::f64::consts::E).abs() < f64::EPSILON);
}

#[test]
fn alloc_multiple_independent_mutations() {
    let arena = Arena::new();
    let a = arena.alloc(10u32);
    let b = arena.alloc(20u32);
    let c = arena.alloc(30u32);

    // Mutate independently
    *a += 1;
    *b += 2;
    *c += 3;
    assert_eq!(*a, 11);
    assert_eq!(*b, 22);
    assert_eq!(*c, 33);
}

#[test]
fn alloc_tuple() {
    let arena = Arena::new();
    let t = arena.alloc((42u16, -7i8, 3.14f32));
    assert_eq!(t.0, 42);
    assert_eq!(t.1, -7);
    assert!((t.2 - 3.14f32).abs() < f32::EPSILON);
    t.0 = 100;
    t.1 = -100;
    assert_eq!(t.0, 100);
    assert_eq!(t.1, -100);
}

#[test]
fn alloc_array() {
    let arena = Arena::new();
    let arr = arena.alloc([1u64, 2, 3, 4, 5]);
    assert_eq!(*arr, [1, 2, 3, 4, 5]);
    arr[2] = 99;
    assert_eq!(arr[2], 99);
}

// --- Alignment ---

#[test]
fn alloc_respects_alignment() {
    let arena = Arena::new();
    // u16 must be 2-byte aligned
    let x = arena.alloc(0u16);
    assert_eq!((x as *const u16 as usize) % 2, 0);
    // u32 must be 4-byte aligned
    let y = arena.alloc(0u32);
    assert_eq!((y as *const u32 as usize) % 4, 0);
    // u64 must be 8-byte aligned
    let z = arena.alloc(0u64);
    assert_eq!((z as *const u64 as usize) % 8, 0);
}

#[test]
fn alloc_alignment_64() {
    let arena = Arena::new();
    // First allocate something small to misalign
    let _a = arena.alloc(1u8);
    // Then allocate the 64-byte aligned type
    let x = arena.alloc(Align64(42));
    assert_eq!((x as *const Align64 as usize) % 64, 0);
    assert_eq!(x.0, 42);
}

#[test]
fn alloc_alignment_128_via_repr() {
    #[repr(align(128))]
    struct Align128(u8);
    let arena = Arena::new();
    let _a = arena.alloc(1u8);
    let x = arena.alloc(Align128(99));
    assert_eq!((x as *const Align128 as usize) % 128, 0);
    assert_eq!(x.0, 99);
}

#[test]
fn alloc_layout_basic() {
    let arena = Arena::new();
    let _pad = arena.alloc(1u8);
    let layout = Layout::from_size_align(3, 64).unwrap();
    let bytes = arena.alloc_layout(layout);
    assert_eq!(bytes.len(), 3);

    let ptr = bytes.as_ptr() as *mut u8;
    assert_eq!(ptr as usize % 64, 0);

    unsafe {
        ptr.write(1);
        ptr.add(1).write(2);
        ptr.add(2).write(3);
    }
    let bytes = unsafe { bytes.as_ref() };
    assert_eq!(bytes, &[1, 2, 3]);
}

#[test]
fn alloc_many_different_sizes_preserves_alignment() {
    let arena = Arena::new();
    // Interleave allocations of different sizes and alignments
    let _u8 = arena.alloc(0u8);
    let u64 = arena.alloc(0u64);
    let _u16 = arena.alloc(0u16);
    let u128_val = arena.alloc(0u128);
    let _u32 = arena.alloc(0u32);
    assert_eq!((u64 as *const u64 as usize) % 8, 0);
    assert_eq!((u128_val as *const u128 as usize) % 16, 0);
}

// --- Filling a chunk (triggers new chunk allocation) ---

#[test]
fn fill_one_chunk_exactly() {
    let arena = Arena::new();
    // Allocate first u128 (triggers initial chunk creation)
    let x = arena.alloc(0u128);
    assert_eq!(*x, 0);
    let mut prev_free = arena.chunk_capacity();
    assert!(prev_free > 0);
    for i in 1..(INITIAL_CHUNK_CAPACITY / 16) as u32 {
        let x = arena.alloc(i as u128);
        assert_eq!(*x, i as u128);
        let curr_free = arena.chunk_capacity();
        assert!(curr_free < prev_free);
        prev_free = curr_free;
    }
    // Should still have some remaining (but less than 16)
    assert!(arena.chunk_capacity() < 16);
}

#[test]
fn alloc_past_one_chunk() {
    let arena = Arena::new();
    // Allocate 600 u64 values.
    // 510 u64 fill one chunk (4080 / 8 = 510); 600 guarantees a second one.
    for i in 0..600u32 {
        let x = arena.alloc(i as u64);
        assert_eq!(*x, i as u64);
    }
}

#[test]
fn alloc_way_past_one_chunk() {
    let arena = Arena::new();
    // Allocate 10,000 u64 values. This should trigger many new chunks.
    for i in 0..10000u64 {
        let x = arena.alloc(i);
        assert_eq!(*x, i);
    }
}

// --- Large allocations (needs_its_own_allocation path) ---

#[test]
fn large_allocation_gets_own_chunk() {
    let arena = Arena::new();

    // Fill up almost the entire chunk so the large alloc won't fit.
    let fill_count = INITIAL_CHUNK_CAPACITY / std::mem::size_of::<u64>() - 1;
    for _ in 0..fill_count {
        let _x = arena.alloc(0u64);
    }
    let free_before_large = arena.chunk_capacity();
    let over_half = INITIAL_CHUNK_CAPACITY / 2 + 1;
    assert!(free_before_large < over_half); // not enough room for the large alloc

    // Allocate something bigger than INITIAL_CHUNK_CAPACITY / 2.
    const BIG: usize = (INITIAL_CHUNK_CAPACITY / 2) + 1;
    let big = arena.alloc(BigAlloc::<BIG>([0; BIG]));

    // Fill with pattern
    for i in 0..BIG {
        big.0[i] = (i % 256) as u8;
    }
    // Check pattern
    for i in 0..BIG {
        assert_eq!(big.0[i], (i % 256) as u8);
    }

    // Current chunk info changed (large-alloc special case was removed).
    assert!(arena.chunk_capacity() > 0);
}

#[test]
fn large_allocation_does_not_affect_current_chunk_space() {
    let arena = Arena::new();

    // Fill chunk so large alloc won't fit in the remaining space.
    let fill_count = INITIAL_CHUNK_CAPACITY / std::mem::size_of::<u64>() - 1;
    for _ in 0..fill_count {
        let _x = arena.alloc(0u64);
    }

    // Allocate large: > INITIAL_CHUNK_CAPACITY / 2
    const BIG: usize = (INITIAL_CHUNK_CAPACITY / 2) + 100;
    let large = arena.alloc(BigAlloc::<BIG>([0xAA; BIG]));
    for byte in large.0.iter() {
        assert_eq!(*byte, 0xAA);
    }

    // Large-alloc special case was removed, so this creates a new chunk.
    // Free space should be non-zero (new chunk has room).
    assert!(arena.chunk_capacity() > 0);
}

#[test]
fn multiple_large_allocations() {
    let arena = Arena::new();

    // Fill chunk so large allocs won't fit.
    let fill_count = INITIAL_CHUNK_CAPACITY / std::mem::size_of::<u64>() - 1;
    for _ in 0..fill_count {
        let _x = arena.alloc(0u64);
    }

    // Allocate multiple large things (no large-alloc special case anymore)
    {
        const SZ0: usize = (INITIAL_CHUNK_CAPACITY / 2) + 1;
        let large = arena.alloc(BigAlloc::<SZ0>([0; SZ0]));
        assert_eq!(large.0[0], 0);
    }
    {
        const SZ1: usize = (INITIAL_CHUNK_CAPACITY / 2) + 101;
        let large = arena.alloc(BigAlloc::<SZ1>([1; SZ1]));
        assert_eq!(large.0[0], 1);
    }
    {
        const SZ2: usize = (INITIAL_CHUNK_CAPACITY / 2) + 201;
        let large = arena.alloc(BigAlloc::<SZ2>([2; SZ2]));
        assert_eq!(large.0[0], 2);
    }

    // All allocations should succeed
    assert!(arena.chunk_capacity() > 0);
}

// --- Clear and reuse ---

#[test]
fn clear_resets_chunk_capacity() {
    let mut arena = Arena::new();
    let _a = arena.alloc(0u64);
    let _b = arena.alloc(0u64);
    let free_before_clear = arena.chunk_capacity();

    arena.clear();

    // After clear, the arena should be reset to full capacity
    assert_eq!(arena.chunk_capacity(), INITIAL_CHUNK_CAPACITY);
    assert!(arena.chunk_capacity() > free_before_clear);
}

#[test]
fn clear_makes_fresh_chunks_available() {
    let mut arena = Arena::new();

    // Allocate past one chunk to create a second chunk
    // 62 u64 fill one chunk; 100 guarantees we create a second one.
    for _ in 0..100 {
        let _x = arena.alloc(0u64);
    }
    assert!(!arena.has_fresh_chunks());

    arena.clear();
    // Now the old chunk should be in fresh_chunks
    assert!(arena.has_fresh_chunks());

    // The current chunk should be fully available
    assert!(arena.chunk_capacity() >= INITIAL_CHUNK_CAPACITY);
}

#[test]
fn clear_then_alloc_reuses_fresh_chunks() {
    let mut arena = Arena::new();

    // Allocate past one chunk (62 u64 fill one; 100 guarantees second)
    for i in 0..100u64 {
        let x = arena.alloc(i);
        assert_eq!(*x, i);
    }

    arena.clear();
    let had_fresh = arena.has_fresh_chunks();
    assert!(had_fresh);

    // Allocate enough to consume the fresh chunk
    // The arena should internally reuse the fresh chunk
    let _a = arena.alloc(42u32);
    let _b = arena.alloc(99u32);
    assert_eq!(*_a, 42);
    assert_eq!(*_b, 99);
}

#[test]
fn repeated_clear_and_alloc() {
    let mut arena = Arena::new();

    for cycle in 0..10 {
        // Allocate many values
        for i in 0..600u64 {
            let x = arena.alloc((i * cycle) as u64);
            assert_eq!(*x, (i * cycle) as u64);
        }
        arena.clear();
        // Every clear resets the arena
        let _first = arena.alloc((cycle + 1) as u64);
    }
}

#[test]
fn clear_multiple_times_builds_chunk_chain() {
    let mut arena = Arena::new();

    // Cycle 1: fill past one chunk, then clear
    for _ in 0..600 {
        let _x = arena.alloc(0u64);
    }
    arena.clear();
    assert!(arena.has_fresh_chunks());

    // Cycle 2: fill past one chunk again, then clear
    for _ in 0..600 {
        let _x = arena.alloc(1u64);
    }
    arena.clear();
    // fresh_chunks should still be non-null (should have accumulated)
    // Actually after clear, the current chunk becomes fresh.
    // So we always have fresh chunks after clearing.
    assert!(arena.has_fresh_chunks());

    // Cycle 3: alloc should work fine
    let x = arena.alloc(42u32);
    assert_eq!(*x, 42);
}

// --- Zero-sized types ---

#[test]
fn alloc_zst_unit() {
    let arena = Arena::new();
    // First alloc may trigger chunk allocation; subsequent ZST allocs
    // should not consume space.
    let _unit = arena.alloc(());
    let free_after_one = arena.chunk_capacity();
    let _unit2 = arena.alloc(());
    let _unit3 = arena.alloc(());
    // ZSTs should not consume space after the initial chunk is allocated
    assert_eq!(arena.chunk_capacity(), free_after_one);
}

#[test]
fn alloc_zst_struct() {
    #[derive(Debug, PartialEq)]
    struct Zst;
    let arena = Arena::new();
    let _z = arena.alloc(Zst);
    let free_after_one = arena.chunk_capacity();
    let _z2 = arena.alloc(Zst);
    assert_eq!(arena.chunk_capacity(), free_after_one);
}

// --- Structs with Drop ---

#[test]
fn alloc_struct_with_drop() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static DROP_COUNT: AtomicU32 = AtomicU32::new(0);

    struct DropCounted {
        _data: u64,
    }
    impl Drop for DropCounted {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }

    {
        let arena = Arena::new();
        let _x = arena.alloc(DropCounted { _data: 42 });
        let _y = arena.alloc(DropCounted { _data: 99 });
        // Drop is NOT called when arena is dropped (this is a bump allocator)
    }
    // Bump allocators don't call drop on individual elements
    assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 0);

    // But the values we allocated should still be valid during arena lifetime
    let arena = Arena::new();
    let a = arena.alloc(DropCounted { _data: 1 });
    let b = arena.alloc(DropCounted { _data: 2 });
    assert_eq!(a._data, 1);
    assert_eq!(b._data, 2);
}

// --- Edge cases ---

#[test]
fn alloc_max_alignment_simd() {
    // Test with a type that has very high alignment (commonly used for SIMD)
    #[repr(align(64))]
    #[allow(dead_code)]
    struct BigAlign([u8; 64]);
    let arena = Arena::new();
    let _pad = arena.alloc(1u8); // misalign
    let x = arena.alloc(BigAlign([0; 64]));
    assert_eq!((x as *const BigAlign as usize) % 64, 0);
}

#[test]
fn alloc_exactly_one_byte() {
    let arena = Arena::new();
    let _ = arena.alloc(0u8); // trigger initial chunk creation
    let free_before = arena.chunk_capacity();
    let x = arena.alloc(0xFFu8);
    assert_eq!(*x, 0xFF);
    assert_eq!(arena.chunk_capacity(), free_before - 1);
}

#[test]
fn alloc_exactly_chunk_size_minus_one() {
    let arena = Arena::new();
    // Allocate (INITIAL_CHUNK_CAPACITY - 1) bytes as u8 array
    // We need to leave room. Let's just fill it up and check the last byte.
    let count = INITIAL_CHUNK_CAPACITY / 8; // u64
    for i in 0..count {
        let x = arena.alloc(i as u64);
        assert_eq!(*x, i as u64);
    }
}

#[test]
fn alloc_after_filling_chunk_uses_new_chunk() {
    let arena = Arena::new();
    // Allocate first u64 (triggers initial chunk creation)
    let x = arena.alloc(0u64);
    assert_eq!(*x, 0);
    let initial_free = arena.chunk_capacity();
    assert!(initial_free > 0);

    // Fill the rest of the first chunk
    let u64_count = INITIAL_CHUNK_CAPACITY / 8;
    let mut pprev_free = initial_free;
    for _ in 1..u64_count {
        let _x = arena.alloc(0u64);
        let r = arena.chunk_capacity();
        assert!(r < pprev_free);
        pprev_free = r;
    }

    // Now allocate one more, which should trigger a new chunk
    // The new chunk has INITIAL_CHUNK_CAPACITY capacity
    let x = arena.alloc(42u64);
    assert_eq!(*x, 42);
}

// --- Fuzz-style random tests ---

#[test]
fn alloc_layout_fuzz() {
    let arena = Arena::new();
    for alignment_pow in 0..=8 {
        let alignment = 2usize.pow(alignment_pow);
        for size in 0..=1024 {
            let layout = Layout::from_size_align(size, alignment).unwrap();
            let bytes = arena.alloc_layout(layout);
            assert_eq!(bytes.len(), size);
            let bytes = bytes.as_ptr() as *mut c_void;
            assert_eq!(bytes as usize % alignment, 0);
            unsafe { memset(bytes, 1, size) };
        }
    }
}

#[test]
fn fuzz_random_types() {
    // Deterministic "random" using a simple LCG
    let mut state: u64 = 0xDEADBEEF_CAFEBABE;
    fn rand(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state >> 33
    }

    let arena = Arena::new();

    // Track pointers to verify they remain non-null.
    let mut ptrs: Vec<*const u8> = Vec::with_capacity(2000);

    for _ in 0..2000 {
        match rand(&mut state) % 6 {
            0 => {
                let x = arena.alloc(rand(&mut state) as u8);
                ptrs.push(x as *const u8 as *const u8);
            }
            1 => {
                let x = arena.alloc(rand(&mut state) as u16);
                ptrs.push(x as *const u16 as *const u8);
            }
            2 => {
                let x = arena.alloc(rand(&mut state) as u32);
                ptrs.push(x as *const u32 as *const u8);
            }
            3 => {
                let x = arena.alloc(rand(&mut state) as u64);
                ptrs.push(x as *const u64 as *const u8);
            }
            4 => {
                let x = arena.alloc([rand(&mut state) as u8; 16]);
                ptrs.push(x.as_ptr());
            }
            5 => {
                let x = arena.alloc([rand(&mut state) as u8; 64]);
                ptrs.push(x.as_ptr());
            }
            _ => unreachable!(),
        }
    }

    for p in &ptrs {
        assert!(!p.is_null());
    }
}

#[test]
fn fuzz_with_alignment_sensitive_types() {
    let mut state: u64 = 0x12345678_9ABCDEF0;
    fn rand(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state >> 33
    }

    let arena = Arena::new();

    for _ in 0..2000 {
        match rand(&mut state) % 5 {
            0 => {
                let x = arena.alloc(rand(&mut state) as u8);
                assert_eq!((x as *const u8 as usize) % 1, 0);
            }
            1 => {
                let _pad = arena.alloc(rand(&mut state) as u8);
                let x = arena.alloc(rand(&mut state) as u16);
                assert_eq!((x as *const u16 as usize) % 2, 0);
            }
            2 => {
                let x = arena.alloc(rand(&mut state) as u32);
                assert_eq!((x as *const u32 as usize) % 4, 0);
            }
            3 => {
                let x = arena.alloc(rand(&mut state) as u64);
                assert_eq!((x as *const u64 as usize) % 8, 0);
            }
            4 => {
                let val = rand(&mut state) as u8;
                let arr = arena.alloc([val; 32]);
                for byte in arr.iter() {
                    assert_eq!(*byte, val);
                }
            }
            _ => unreachable!(),
        }
    }
}

// --- Memory exhaustion resistance ---

#[test]
fn alloc_very_many_tiny_things() {
    let arena = Arena::new();
    // Allocate 100,000 u8 values. Should work fine.
    for i in 0..100_000u32 {
        let x = arena.alloc(i as u8);
        assert_eq!(*x, i as u8);
    }
}

#[test]
fn alloc_very_many_medium_things() {
    let arena = Arena::new();
    // Allocate 10,000 [u8; 10] arrays
    for i in 0..10_000u16 {
        let allocated = arena.alloc([(i % 256) as u8; 10]);
        assert_eq!(allocated[0], (i % 256) as u8);
    }
}

// --- Pointer stability ---

#[test]
fn pointers_are_stable_across_allocations() {
    let arena = Arena::new();

    let a = arena.alloc(1u64) as *const u64;
    let b = arena.alloc(2u64) as *const u64;

    // Allocate many more things
    for _ in 0..500 {
        let _x = arena.alloc(0u64);
    }

    // The original pointers should still be valid
    unsafe {
        assert_eq!(*a, 1);
        assert_eq!(*b, 2);
    }
}

#[test]
fn pointers_are_stable_after_clear_on_current_chunk() {
    let mut arena = Arena::new();

    // Allocate some values, record pointer
    let _a_ptr = arena.alloc(42u64) as *const u64;

    // Don't overflow the first chunk -- we want to stay in it
    arena.clear();

    let b = arena.alloc(99u64);

    // a_ptr and b might be different since arena memory model goes
    // from top down, and clear resets top, so the new allocation could
    // be at the same address. We just verify b is correct.
    assert_eq!(*b, 99);
}

// --- Type parameterization (generic T works) ---

#[test]
fn alloc_generic_struct() {
    #[derive(Debug, PartialEq)]
    struct Pair<T, U>(T, U);

    let arena = Arena::new();
    let p1 = arena.alloc(Pair(42u32, "hello"));
    let p2 = arena.alloc(Pair(3.14f64, true));

    assert_eq!(p1.0, 42);
    assert_eq!(p1.1, "hello");
    assert_eq!(p2.0, 3.14);
    assert_eq!(p2.1, true);
}

#[test]
fn alloc_option() {
    let arena = Arena::new();
    let some = arena.alloc(Some(42u32));
    let none: &mut Option<u32> = arena.alloc(None);

    assert_eq!(*some, Some(42));
    assert_eq!(*none, None);

    *none = Some(7);
    assert_eq!(*none, Some(7));
}

#[test]
fn alloc_result() {
    let arena = Arena::new();
    let ok: &mut Result<u32, &str> = arena.alloc(Ok(100));
    let err: &mut Result<u32, &str> = arena.alloc(Err("oops"));

    assert_eq!(*ok, Ok(100));
    assert_eq!(*err, Err("oops"));
}

// --- Clear edge cases ---

#[test]
fn clear_on_fresh_arena() {
    let mut arena = Arena::new();
    let free_before = arena.chunk_capacity();
    assert!(!arena.has_fresh_chunks());

    arena.clear();

    // Clearing a fresh arena should still have the same remaining
    assert_eq!(arena.chunk_capacity(), free_before);
    // No fresh chunks were added (the only chunk is current, not moved)
    assert!(!arena.has_fresh_chunks());
}

#[test]
fn clear_then_clear_again() {
    let mut arena = Arena::new();
    // Consume less than a chunk, then clear
    for _ in 0..INITIAL_CHUNK_CAPACITY / std::mem::size_of::<u64>() {
        let _x = arena.alloc(0u64);
    }
    arena.clear();
    let free_after_first_clear = arena.chunk_capacity();
    assert!(!arena.has_fresh_chunks()); // only one chunk, still current

    // Consume more space
    for _ in 0..INITIAL_CHUNK_CAPACITY / std::mem::size_of::<u64>() {
        let _x = arena.alloc(0u64);
    }

    arena.clear();
    assert_eq!(arena.chunk_capacity(), free_after_first_clear);
    assert!(!arena.has_fresh_chunks()); // Didn't need to allocate another...
}

#[test]
fn clear_and_reuse_multiple_chunks_round_robin() {
    let mut arena = Arena::new();

    // Phase 1: use 3 chunks worth of space
    let u64_per_chunk = INITIAL_CHUNK_CAPACITY / 8;
    for _ in 0..u64_per_chunk * 3 {
        let _x = arena.alloc(0u64);
    }

    arena.clear();
    assert!(arena.has_fresh_chunks());

    // Phase 2: use 2 more chunks worth
    for _ in 0..u64_per_chunk * 2 {
        let _x = arena.alloc(1u64);
    }

    arena.clear();

    // Phase 3: allocate a few, they should work
    for i in 0..100u32 {
        let x = arena.alloc(i);
        assert_eq!(*x, i);
    }
}

// --- Large allocation edge cases ---

#[test]
fn large_allocation_exactly_half_chunk_does_not_trigger_own_chunk() {
    // layout.size() > INITIAL_CHUNK_CAPACITY / 2 is strict >, so
    // exactly half (2040) does NOT get its own chunk.
    // But if there's room in the chunk, it won't hit alloc_more anyway.
    // So we fill the chunk first to force the slow path, then alloc.
    let arena = Arena::new();
    const HALF: usize = INITIAL_CHUNK_CAPACITY / 2;

    // Fill chunk so remaining space is less than HALF.
    let fill_count = INITIAL_CHUNK_CAPACITY / std::mem::size_of::<u64>() - 1;
    for _ in 0..fill_count {
        let _x = arena.alloc(0u64);
    }
    let free_before = arena.chunk_capacity();
    assert!(free_before < HALF);

    // Alloc HALF bytes. Triggers alloc_more, but HALF > HALF is false,
    // so it goes to the normal new-chunk path and updates bottom/top.
    let _allocated = arena.alloc(BigAlloc::<HALF>([0x42; HALF]));

    // Since we went through the normal new-chunk path, a new chunk was
    // created and the HALF-byte alloc was placed in it.
    assert!(arena.chunk_capacity() > 0);
}

#[test]
fn large_allocation_just_over_half_chunk_gets_own_chunk() {
    let arena = Arena::new();
    const OVER_HALF: usize = INITIAL_CHUNK_CAPACITY / 2 + 1;

    // Fill chunk so the large alloc won't fit.
    let fill_count = INITIAL_CHUNK_CAPACITY / std::mem::size_of::<u64>() - 1;
    for _ in 0..fill_count {
        let _x = arena.alloc(0u64);
    }
    let free_before = arena.chunk_capacity();
    assert!(free_before < OVER_HALF);

    // Allocate the large value. This does NOT get its own chunk anymore
    // (the large-alloc special case was removed). Instead it gets a new
    // chunk via the normal slow path, which updates the current chunk.
    let _allocated = arena.alloc(BigAlloc::<OVER_HALF>([0x42; OVER_HALF]));

    // The current chunk is now the new chunk, so free space changed.
    assert!(arena.chunk_capacity() > 0);
}

// --- Drop works without crashing (smoke test) ---

#[test]
fn drop_arena_does_not_crash() {
    let arena = Arena::new();
    // Allocate various things
    let _a = arena.alloc(1u32);
    let _b = arena.alloc(2u64);
    let _c = arena.alloc([0u8; 100]);
    // Let it drop
}

#[test]
fn drop_after_many_allocations() {
    let arena = Arena::new();
    for i in 0..5000u64 {
        let _x = arena.alloc(i);
    }
    // Let it drop
}

#[test]
fn drop_after_clear() {
    let mut arena = Arena::new();
    for _ in 0..500 {
        let _x = arena.alloc(0u64);
    }
    arena.clear();
    for _ in 0..500 {
        let _x = arena.alloc(1u64);
    }
    // Let it drop (with fresh_chunks populated)
}

#[test]
fn drop_after_large_allocations() {
    let arena = Arena::new();

    // Fill chunk so large allocs won't fit.
    let fill_count = INITIAL_CHUNK_CAPACITY / std::mem::size_of::<u64>() - 1;
    for _ in 0..fill_count {
        let _x = arena.alloc(0u64);
    }

    // Multiple large allocations (large-alloc special case was removed,
    // so these go through normal slow path and use new chunks).
    const BIG: usize = INITIAL_CHUNK_CAPACITY / 2 + 50;
    {
        let _x = arena.alloc(BigAlloc::<BIG>([0; BIG]));
    }
    {
        let _x = arena.alloc(BigAlloc::<BIG>([0; BIG]));
    }

    // Drop should clean up the main chain and the large-allocation sub-chain
}

#[test]
fn drop_empty_arena() {
    // Just create and drop
    let _arena = Arena::new();
}

// --- Stress test ---

#[test]
fn stress_test_mixed_allocation_patterns() {
    let mut state: u64 = 0xBEEF_FACE;
    fn rand(state: &mut u64) -> u64 {
        *state = state.wrapping_mul(1103515245).wrapping_add(12345);
        *state
    }

    let mut arena = Arena::new();

    for _cycle in 0..5 {
        // Phase 1: many small allocations
        for _ in 0..200 {
            match rand(&mut state) % 5 {
                0 => {
                    let _ = arena.alloc(rand(&mut state) as u8);
                }
                1 => {
                    let _ = arena.alloc((rand(&mut state) as u32).wrapping_mul(42));
                }
                2 => {
                    let _ = arena.alloc(rand(&mut state) as u64);
                }
                3 => {
                    let _ = arena.alloc([rand(&mut state) as u8; 8]);
                }
                4 => {
                    let _ = arena.alloc([rand(&mut state) as u8; 32]);
                }
                _ => unreachable!(),
            }
        }

        // Phase 2: a few large allocations (use BigAlloc with fixed sizes)
        let _large1 = arena.alloc(BigAlloc::<{ INITIAL_CHUNK_CAPACITY / 2 + 50 }>(
            [0xAA; INITIAL_CHUNK_CAPACITY / 2 + 50],
        ));
        let _large2 = arena.alloc(BigAlloc::<{ INITIAL_CHUNK_CAPACITY / 2 + 200 }>(
            [0xBB; INITIAL_CHUNK_CAPACITY / 2 + 200],
        ));
        let _large3 = arena.alloc(BigAlloc::<{ INITIAL_CHUNK_CAPACITY / 2 + 300 }>(
            [0xCC; INITIAL_CHUNK_CAPACITY / 2 + 300],
        ));

        // Phase 3: more small allocations
        for _ in 0..100 {
            match rand(&mut state) % 3 {
                0 => {
                    let _ = arena.alloc(rand(&mut state) as u16);
                }
                1 => {
                    let _ = arena.alloc([rand(&mut state) as u32; 4]);
                }
                2 => {
                    let _ = arena.alloc([0xFFu8; 16]);
                }
                _ => unreachable!(),
            }
        }

        arena.clear();
    }
}

// --- Consecutive clears without intervening allocations ---

#[test]
fn multiple_clears_without_allocs() {
    let mut arena = Arena::new();
    let _x = arena.alloc(42u32);

    arena.clear();
    arena.clear();
    arena.clear();

    let y = arena.alloc(99u32);
    assert_eq!(*y, 99);
}

// --- Allocating arrays of different sizes ---

#[test]
fn alloc_fixed_size_array() {
    let arena = Arena::new();
    let arr = arena.alloc([1u8, 2, 3, 4, 5]);
    assert_eq!(*arr, [1, 2, 3, 4, 5]);
    arr[2] = 99;
    assert_eq!(arr[2], 99);
}

#[test]
fn alloc_medium_array() {
    let arena = Arena::new();
    let mut data = [0u8; 256];
    for i in 0..256 {
        data[i] = i as u8;
    }
    let arr = arena.alloc(data);
    for i in 0..256 {
        assert_eq!(arr[i], i as u8);
    }
}

// --- alloc returns a valid mutable reference (write check) ---

#[test]
fn alloc_returns_unique_mutable_reference() {
    let arena = Arena::new();
    let a = arena.alloc(0u32);
    let b = arena.alloc(0u32);
    *a = 42;
    *b = 99;
    // If the references weren't unique, we'd see UB (but this compiles, so they are)
    assert_eq!(*a, 42);
    assert_eq!(*b, 99);
}

// This surfaced an error through miri, where we were creating
// a mutable reference to static data.
#[test]
fn bespoke_allocation_first() {
    let arena = Arena::new();
    arena.alloc(INITIAL_CHUNK_CAPACITY + 1);
}

#[test]
fn can_clear_empty_arena() {
    let mut arena = Arena::new();
    arena.clear();
}

// --- alloc_str ---

#[test]
fn alloc_str_basic() {
    let arena = Arena::new();
    let s = arena.alloc_str("hello world");
    assert_eq!(s, "hello world");
    // Verify mutability.
    s.make_ascii_uppercase();
    assert_eq!(s, "HELLO WORLD");
}

#[test]
fn alloc_str_empty() {
    let arena = Arena::new();
    let s = arena.alloc_str("");
    assert_eq!(s, "");
    assert_eq!(s.len(), 0);
}

#[test]
fn alloc_str_multi_byte() {
    let arena = Arena::new();
    let s = arena.alloc_str("a̐éõ🦀");
    assert_eq!(s, "a̐éõ🦀");
}

#[test]
fn alloc_str_long() {
    let arena = Arena::new();
    let data = "x".repeat(2000);
    let s = arena.alloc_str(&data);
    assert_eq!(*s, data);
    let mut arena = Arena::new();
    let s1 = arena.alloc_str("first");
    assert_eq!(s1, "first");
    arena.clear();
    let s2 = arena.alloc_str("second");
    assert_eq!(s2, "second");
}

#[test]
fn alloc_str_past_one_chunk() {
    let arena = Arena::new();
    // Fill the 32-byte chunk.
    arena.alloc_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    // This forces alloc_more.
    let s = arena.alloc_str("hello from second chunk");
    assert_eq!(s, "hello from second chunk");
}

// --- alloc_slice_copy ---

#[test]
fn alloc_slice_copy_basic() {
    let arena = Arena::new();
    let slice = arena.alloc_slice_copy(&[1, 2, 3, 4]);
    assert_eq!(slice, [1, 2, 3, 4]);
    slice[0] = 99;
    assert_eq!(slice, [99, 2, 3, 4]);
}

#[test]
fn alloc_slice_copy_empty() {
    let arena = Arena::new();
    let slice = arena.alloc_slice_copy(&[] as &[u8]);
    assert!(slice.is_empty());
}

#[test]
fn alloc_slice_copy_long() {
    let arena = Arena::new();
    let data = vec![0xABu8; 1024];
    let slice = arena.alloc_slice_copy(&data);
    assert_eq!(slice, data);
}

#[test]
fn alloc_slice_copy_with_clear() {
    let mut arena = Arena::new();
    let s1 = arena.alloc_slice_copy(b"first");
    assert_eq!(s1, b"first");
    arena.clear();
    let s2 = arena.alloc_slice_copy(b"second");
    assert_eq!(s2, b"second");
}

#[test]
fn alloc_slice_copy_past_one_chunk() {
    let arena = Arena::new();
    // Fill the 32-byte chunk.
    arena.alloc_slice_copy(&[0; 30]);
    // This forces alloc_more.
    let slice = arena.alloc_slice_copy(b"hello from second chunk");
    assert_eq!(slice, b"hello from second chunk");
}

#[test]
fn arena_forwarding_alloc() {
    let root = Arena::new();
    let x = root.alloc(42u32);
    assert_eq!(*x, 42);
    *x = 99;
    assert_eq!(*x, 99);
}

#[test]
fn arena_forwarding_alloc_with() {
    let root = Arena::new();
    let x = root.alloc_with(|| [1, 2, 3]);
    assert_eq!(x, &[1, 2, 3]);
}

#[test]
fn arena_forwarding_alloc_slice_copy() {
    let root = Arena::new();
    let slice = root.alloc_slice_copy(&[10, 20, 30]);
    assert_eq!(slice, &[10, 20, 30]);
}

#[test]
fn arena_forwarding_alloc_str() {
    let root = Arena::new();
    let s = root.alloc_str("hello");
    assert_eq!(s, "hello");
}

#[test]
fn arena_forwarding_allocated_bytes() {
    let root = Arena::new();
    let _ = root.alloc(42u32);
    assert!(root.allocated_bytes() > 0);
}

#[test]
fn arena_forwarding_sized_slice() {
    let root = Arena::new();
    let arr = root.alloc_sized_slice_fill_default::<u32, 4>();
    assert_eq!(arr, &[0, 0, 0, 0]);
}

// --- with_scope ---

#[test]
fn with_scope_basic() {
    let mut arena = Arena::new();
    arena.with_scope(|scope| {
        let x = scope.alloc(42u32);
        assert_eq!(*x, 42);
        *x = 99;
        assert_eq!(*x, 99);
    });
}

#[test]
fn with_scope_nested() {
    let mut arena = Arena::new();
    arena.alloc(1u32);
    let cap = arena.chunk_capacity();

    arena.with_scope(|outer_scope| {
        let a = outer_scope.alloc(2u32);

        outer_scope.with_scope(|inner_scope| {
            let inner = inner_scope.alloc(3u32);
            assert_eq!(*inner, 3);
            for i in 0..10 {
                let v = inner_scope.alloc(i as u128);
                assert_eq!(*v, i as u128);
            }
        });

        // After inner scope reverts, allocate in outer scope normally
        let after = outer_scope.alloc(4u32);
        assert_eq!(*after, 4);

        // outer_scope is an ArenaRef, which allows for allocations to
        // be used across scope introduction/elimination.
        assert_eq!(*a, 2);
    });

    // After both scopes, arena state is intact
    assert_eq!(arena.chunk_capacity(), cap);
}

// This is not behaviour that's exposed via the safe API
// Do not do this.
#[test]
fn with_scope_outer_preserved() {
    let mut arena = Arena::new();

    // Allocate before the scope, snapshot via raw pointer.
    // with_scope takes &mut self, so &mut T from alloc can't be held.
    let x_addr = arena.alloc(10u32) as *mut u32;
    let y_addr = arena.alloc(20u32) as *mut u32;
    let b_addr = arena.alloc(0xFFu8) as *mut u8;

    // Scope allocates a lot, potentially crossing chunk boundaries
    arena.with_scope(|scope| {
        for i in 0..30 {
            let v = scope.alloc(i as u128);
            *v = (i * 2) as u128;
            assert_eq!(*v, (i * 2) as u128);
        }
    });

    // Memory is still there — the scope restored the checkpoint
    assert_eq!(unsafe { *x_addr }, 10);
    assert_eq!(unsafe { *y_addr }, 20);
    assert_eq!(unsafe { *b_addr }, 0xFF);

    // Arena is still usable after the scope
    let new_int = arena.alloc(999u32);
    assert_eq!(*new_int, 999);
}

#[test]
fn with_scope_multiple_sequential() {
    let mut arena = Arena::new();
    let arena = arena.as_arena_ref_mut();

    // Allocate before any scope (raw ptr to survive &mut self)
    // This isn't an intended use case, but happens to be safe...
    let a_addr = arena.alloc(1u32) as *mut u32;
    assert_eq!(unsafe { *a_addr }, 1);

    // First scope
    arena.with_scope(|scope| {
        let x = scope.alloc(100u32);
        *x = *x + 1;
        assert_eq!(*x, 101);
    });

    // Memory intact after first scope
    assert_eq!(unsafe { *a_addr }, 1);

    // Second scope
    arena.with_scope(|scope| {
        let y = scope.alloc(200u32);
        *y = *y + 2;
        assert_eq!(*y, 202);
    });

    // Memory intact after second scope
    assert_eq!(unsafe { *a_addr }, 1);

    // Allocate more after scopes
    let b = arena.alloc(2u32);
    assert_eq!(*b, 2);
}

#[test]
fn with_scope_reuses_chunks() {
    let mut arena = Arena::new();

    // First alloc gives us a chunk
    arena.alloc(1u32);
    let cap_after_first = arena.chunk_capacity();
    assert!(cap_after_first > 0);

    assert!(!arena.has_fresh_chunks());
    arena.with_scope(|scope| {
        scope.alloc(0u128);
    });
    assert_eq!(arena.chunk_capacity(), cap_after_first);
    // This wasn't enough to create a new chunk...
    assert!(!arena.has_fresh_chunks());

    assert!(!arena.has_fresh_chunks());
    arena.with_scope(|scope| {
        // Fill current chunk to force allocation of another
        for _ in 0..100 {
            scope.alloc(0u128);
        }
    });

    // After scope, chunk_capacity should be back to what
    // we had before the scope (the extra chunk is reused, not freed)
    assert_eq!(arena.chunk_capacity(), cap_after_first);
    assert!(arena.has_fresh_chunks());
}

// --- alloc_str_lit ---

#[test]
fn alloc_str_lit_basic() {
    let arena = Arena::new();
    let s = arena.alloc_str_lit("hello world");
    assert_eq!(s, "hello world");
    // Verify mutability.
    s.make_ascii_uppercase();
    assert_eq!(s, "HELLO WORLD");
}

#[test]
fn alloc_str_lit_empty() {
    let arena = Arena::new();
    let s = arena.alloc_str_lit("");
    assert_eq!(s, "");
    assert_eq!(s.len(), 0);
}

#[test]
fn alloc_str_lit_multi_byte() {
    let arena = Arena::new();
    let s = arena.alloc_str_lit("a̐éõ🦀");
    assert_eq!(s, "a̐éõ🦀");
}

#[test]
fn alloc_str_lit_with_clear() {
    let mut arena = Arena::new();
    let s1 = arena.alloc_str_lit("first");
    assert_eq!(s1, "first");
    arena.clear();
    let s2 = arena.alloc_str_lit("second");
    assert_eq!(s2, "second");
}

#[test]
fn alloc_str_lit_past_one_chunk() {
    let arena = Arena::new();
    // Fill the 32-byte chunk.
    arena.alloc_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    // This forces alloc_more.
    let s = arena.alloc_str_lit("hello from second chunk");
    assert_eq!(s, "hello from second chunk");
}

#[test]
fn alloc_str_lit_many() {
    let arena = Arena::new();
    // Allocate many string literals to exercise the fast path heavily.
    for _ in 0..1000 {
        let s = arena.alloc_str_lit("fast path");
        assert_eq!(s, "fast path");
    }
}

// --- alloc_slice_lit_copy ---

#[test]
fn alloc_slice_lit_copy_basic() {
    let arena = Arena::new();
    let slice = arena.alloc_slice_lit_copy(&[1, 2, 3, 4]);
    assert_eq!(slice, [1, 2, 3, 4]);
    slice[0] = 99;
    assert_eq!(slice, [99, 2, 3, 4]);
}

#[test]
fn alloc_slice_lit_copy_empty() {
    let arena = Arena::new();
    let slice = arena.alloc_slice_lit_copy(&[] as &[u8]);
    assert!(slice.is_empty());
}

#[test]
fn alloc_slice_lit_copy_with_clear() {
    let mut arena = Arena::new();
    let s1 = arena.alloc_slice_lit_copy(b"first");
    assert_eq!(s1, b"first");
    arena.clear();
    let s2 = arena.alloc_slice_lit_copy(b"second");
    assert_eq!(s2, b"second");
}

#[test]
fn alloc_slice_lit_copy_past_one_chunk() {
    let arena = Arena::new();
    // Fill the 32-byte chunk.
    arena.alloc_slice_copy(&[0; 30]);
    // This forces alloc_more.
    let slice = arena.alloc_slice_lit_copy(b"hello from second chunk");
    assert_eq!(slice, b"hello from second chunk");
}

#[test]
fn alloc_slice_lit_copy_many() {
    let arena = Arena::new();
    // Allocate many slice literals to exercise the fast path heavily.
    for _ in 0..1000 {
        let s = arena.alloc_slice_lit_copy(&[1u8, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(s, [1, 2, 3, 4, 5, 6, 7, 8]);
    }
}

#[test]
fn alloc_slice_lit_copy_f64() {
    let arena = Arena::new();
    let slice = arena.alloc_slice_lit_copy(&[1.0f64, 2.5, 3.14]);
    assert!((slice[0] - 1.0).abs() < f64::EPSILON);
    assert!((slice[1] - 2.5).abs() < f64::EPSILON);
    assert!((slice[2] - 3.14).abs() < f64::EPSILON);
    slice[1] = 99.9;
    assert!((slice[1] - 99.9).abs() < f64::EPSILON);
}

// --- Alloc functions do not double-drop source values ---
//
// Alloc functions that move values into the arena via ptr::write or
// ptr::copy_nonoverlapping must not drop the source values, otherwise
// types with Drop impls (e.g. String, Vec) would have their heap
// buffers freed while the arena copy still references them.

#[test]
fn alloc_sized_slice_with_does_not_drop_source() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static DROP_COUNT: AtomicU32 = AtomicU32::new(0);
    struct DC {
        _data: u64,
    }
    impl Drop for DC {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }

    // alloc_sized_slice_with wraps f() in ManuallyDrop.
    DROP_COUNT.store(0, Ordering::Relaxed);
    let arena = Arena::new();
    let _arr = arena.alloc_sized_slice_with::<DC, 4, _>(|| {
        [
            DC { _data: 1 },
            DC { _data: 2 },
            DC { _data: 3 },
            DC { _data: 4 },
        ]
    });
    assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 0);
}

#[test]
fn alloc_with_does_not_drop_source() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static DROP_COUNT: AtomicU32 = AtomicU32::new(0);
    struct DC {
        _data: u64,
    }
    impl Drop for DC {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }

    // alloc_with uses ptr::write, which forgets the source.
    DROP_COUNT.store(0, Ordering::Relaxed);
    let arena = Arena::new();
    let _val = arena.alloc_with(|| DC { _data: 42 });
    assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 0);
}

#[test]
fn alloc_does_not_drop_source() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static DROP_COUNT: AtomicU32 = AtomicU32::new(0);
    struct DC {
        _data: u64,
    }
    impl Drop for DC {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }

    // alloc delegates to alloc_with.
    DROP_COUNT.store(0, Ordering::Relaxed);
    let arena = Arena::new();
    let _val = arena.alloc(DC { _data: 99 });
    assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 0);
}

#[test]
fn alloc_sized_slice_fill_with_does_not_drop_source() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static DROP_COUNT: AtomicU32 = AtomicU32::new(0);
    struct DC {
        _data: u64,
    }
    impl Drop for DC {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }

    // Uses ptr::write per element.
    DROP_COUNT.store(0, Ordering::Relaxed);
    let arena = Arena::new();
    let _arr = arena.alloc_sized_slice_fill_with::<DC, 4, _>(|_i| DC { _data: 1 });
    assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 0);
}

#[test]
fn alloc_slice_fill_with_does_not_drop_source() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static DROP_COUNT: AtomicU32 = AtomicU32::new(0);
    struct DC {
        _data: u64,
    }
    impl Drop for DC {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }

    // Same as alloc_sized_slice_fill_with but runtime-sized.
    DROP_COUNT.store(0, Ordering::Relaxed);
    let arena = Arena::new();
    let _arr = arena.alloc_slice_fill_with::<DC, _>(4, |_i| DC { _data: 1 });
    assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 0);
}

#[test]
fn alloc_slice_try_fill_with_does_not_drop_source_on_ok() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static DROP_COUNT: AtomicU32 = AtomicU32::new(0);
    struct DC {
        _data: u64,
    }
    impl Drop for DC {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }

    // On success, ptr::write per element prevents source drops.
    DROP_COUNT.store(0, Ordering::Relaxed);
    let arena = Arena::new();
    let r: Result<&mut [DC], ()> = arena.alloc_slice_try_fill_with(4, |_i| Ok(DC { _data: 1 }));
    let _arr = r.unwrap();
    assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 0);
}

// --- Panic: over-allocation (capacity overflow) ---
//
// Functions that take a `len: usize` parameter pass it to Layout::array,
// which panics when size_of::<T>() * len > isize::MAX.
//
// Functions that take &[T] (alloc_slice_copy, alloc_slice_clone,
// alloc_slice_lit_copy, alloc_str, alloc_str_lit) cannot trigger this
// panic because a valid Rust slice reference already has total byte size
// <= isize::MAX, so the Layout::array precondition is always satisfied.

// alloc_slice_fill_with: T = u8, len > isize::MAX triggers overflow.
#[test]
#[should_panic]
fn over_allocation_alloc_slice_fill_with_u8() {
    let arena = Arena::new();
    let too_big = (isize::MAX as usize).saturating_add(1);
    let _: &mut [u8] = arena.alloc_slice_fill_with(too_big, |_| 0u8);
}

// alloc_slice_fill_with: T = u64, len > isize::MAX / 8 triggers overflow.
#[test]
#[should_panic]
fn over_allocation_alloc_slice_fill_with_u64() {
    let arena = Arena::new();
    let too_big = (isize::MAX as usize / 8).saturating_add(1);
    let _: &mut [u64] = arena.alloc_slice_fill_with(too_big, |_| 0);
}

// alloc_slice_fill_default with an impossibly large length.
#[test]
#[should_panic]
fn over_allocation_alloc_slice_fill_default() {
    let arena = Arena::new();
    let too_big = (isize::MAX as usize).saturating_add(1);
    let _: &mut [u8] = arena.alloc_slice_fill_default(too_big);
}

// alloc_slice_try_fill_with with an impossibly large length.
#[test]
#[should_panic]
fn over_allocation_alloc_slice_try_fill_with() {
    let arena = Arena::new();
    let too_big = (isize::MAX as usize).saturating_add(1);
    let _: Result<&mut [u8], ()> = arena.alloc_slice_try_fill_with(too_big, |_| Ok(0u8));
}
