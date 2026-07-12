use stumpalo::Arena;

fn main() {
    let mut root = Arena::new();

    // Arena::with_scope takes &self (unlike ArenaRef::with_scope which
    // takes &mut self), so holding &mut references from alloc() across
    // the call compiles successfully.
    let x = root.alloc(42u32);
    assert_eq!(*x, 42);

    root.with_scope(|scope| {
        let y = scope.alloc(1u32);
        assert_eq!(*y, 1);
    });

    // The pre-scope reference remains valid after the scope returns.
    assert_eq!(*x, 42);
}
