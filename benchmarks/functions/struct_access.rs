const N: i64 = 1000000;

struct Point {
    x: i64,
    y: i64,
}

fn work(n: i64) -> i64 {
    let point = std::hint::black_box(Point { x: 3, y: 4 });
    let mut total = 0i64;
    for _ in 0..n {
        let p = std::hint::black_box(&point);
        total += p.x * p.y;
    }
    total
}

fn main() {
    println!("{}", work(N));
}
