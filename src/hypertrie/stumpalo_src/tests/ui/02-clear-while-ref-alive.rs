use stumpalo::Arena;

fn main() {
    let mut arena = Arena::new();
    let a = arena.alloc(3u32);
    arena.clear();
    println!("{}", a);
}
