use stumpalo::Arena;

fn main() {
    let root = Arena::new();
    let a = root.alloc(3u32);
    drop(root);
    let _b = a;
}
