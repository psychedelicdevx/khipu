const N: i64 = 1000000;

fn work(n: i64) -> i64 {
    let (mut a, mut b) = (0i64, 1i64);
    for _ in 0..n {
        a += b;
        b += 1;
    }
    a % 1000
}

fn main() {
    println!("{}", work(std::hint::black_box(N)));
}
