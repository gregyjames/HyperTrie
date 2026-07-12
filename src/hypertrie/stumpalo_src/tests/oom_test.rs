use stumpalo::Arena;

#[test]
#[should_panic(expected = "OOM")]
fn oom_on_chunk_allocation() {
    let arena = Arena::new();
    let huge_len = isize::MAX as usize;
    let _: &mut [u8] = arena.alloc_slice_fill_with(huge_len, |_| 0u8);
}
