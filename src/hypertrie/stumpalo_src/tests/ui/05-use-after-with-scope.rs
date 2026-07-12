use stumpalo::Arena;

fn main() {
    let mut root = Arena::new();
    let arena = root.as_arena_ref_mut();
    let mut out: Option<&mut u32> = None;
    arena.with_scope(|scope| {
        out = Some(scope.alloc(42u32));
    });
    println!("{}", out.unwrap());
}
