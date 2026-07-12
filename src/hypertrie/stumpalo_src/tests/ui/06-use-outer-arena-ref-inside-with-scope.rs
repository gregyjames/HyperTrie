use stumpalo::Arena;

fn main() {
    let mut arena = Arena::new();
    arena.with_scope(|scope| {
        arena.alloc(1u32);
    });
}
