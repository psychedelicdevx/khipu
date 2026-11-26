const N: i64 = 200000;

struct Point {
    x: i64,
    y: i64,
}

fn point(x: i64, y: i64) -> Point {
    Point { x, y }
}

fn main() {
    let mut total = 0i64;
    for i in 0..N {
        let p = point(i, i + 1);
        total = total + p.x + p.y * 2;
    }
    println!("{}", total);
}
