use stumpalo::Arena;

fn main() {
    let mut arena = Arena::new();
    let r: &mut u32 = arena.with_scope(|scope| scope.alloc(42u32));
    println!("{}", r);
}
