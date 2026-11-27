const N: i64 = 1000000;

#[derive(Clone, Copy)]
struct Point {
    x: i64,
    y: i64,
}

fn make(x: i64, y: i64) -> Point {
    std::hint::black_box(Point { x, y })
}

fn area(p: Point) -> i64 {
    p.x * p.y
}

fn work(n: i64) -> i64 {
    let point = std::hint::black_box(make(3, 4));
    let mut total = 0i64;
    for _ in 0..n {
        let p = std::hint::black_box(&point);
        total += area(std::hint::black_box(*p));
    }
    total
}

fn main() {
    println!("{}", work(N));
}
