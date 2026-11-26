use std::sync::atomic::{AtomicI64, Ordering};

const N: i64 = 1000000;

static COUNTER: AtomicI64 = AtomicI64::new(0);

#[inline(never)]
fn bump() {
    COUNTER.fetch_add(1, Ordering::Relaxed);
}

fn main() {
    for _ in 0..N {
        bump();
    }
    println!("{}", COUNTER.load(Ordering::Relaxed));
}
