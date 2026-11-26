const N: i64 = 60;

fn main() {
    let n = N as f64;
    let mut inside = 0i64;
    for py in 0..N {
        for px in 0..N {
            let cx = px as f64 * 3.0 / n - 2.0;
            let cy = py as f64 * 2.0 / n - 1.0;
            let (mut x, mut y) = (0.0f64, 0.0f64);
            let mut i = 0;
            while i < 50 && x * x + y * y <= 4.0 {
                let t = x * x - y * y + cx;
                y = 2.0 * x * y + cy;
                x = t;
                i += 1;
            }
            if i == 50 {
                inside += 1;
            }
        }
    }
    println!("{}", inside);
}
