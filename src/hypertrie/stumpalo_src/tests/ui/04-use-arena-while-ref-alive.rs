use stumpalo::Arena;

fn main() {
    let mut arena = Arena::new();
    let r = arena.as_arena_ref_mut();
    let a = arena.alloc(1u32);
    r.alloc(2u32);
}
