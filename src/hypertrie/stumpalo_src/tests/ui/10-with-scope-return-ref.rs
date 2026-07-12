use stumpalo::Arena;

fn main() {
    let mut root = Arena::new();
    let arena = root.as_arena_ref_mut();
    let r: &mut u32 = arena.with_scope(|scope| scope.alloc(42u32));
    println!("{}", r);
}
