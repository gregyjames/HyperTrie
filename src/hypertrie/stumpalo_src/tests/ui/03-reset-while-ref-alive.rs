use stumpalo::Arena;

fn main() {
    let mut root = Arena::new();
    let arena = root.as_arena_ref_mut();
    let a = arena.alloc(3u32);
    arena.reset();
    println!("{}", a);
}
