const N: i64 = 500000;

fn main() {
    let mut count = 0i64;
    let mut next = || {
        count += 1;
        count
    };
    let mut last = 0i64;
    for _ in 0..N {
        last = std::hint::black_box(next());
    }
    println!("{}", last);
}
