use stumpalo::Arena;

fn main() {
    let mut arena = Arena::new();
    let mut out: Option<&mut u32> = None;
    arena.with_scope(|scope| {
        out = Some(scope.alloc(42u32));
    });
    println!("{}", out.unwrap());
}
