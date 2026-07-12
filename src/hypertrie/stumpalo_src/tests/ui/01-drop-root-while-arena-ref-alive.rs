use stumpalo::Arena;

fn main() {
    let mut root = Arena::new();
    let arena = root.as_arena_ref_mut();
    drop(root);
    let _a = arena.alloc(3u32);
}
