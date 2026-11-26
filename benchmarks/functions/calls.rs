const N: i64 = 300000;

#[inline(never)]
fn identity(x: i64) -> i64 {
    x
}

fn main() {
    let mut total = 0i64;
    for i in 0..N {
        total += identity(i);
    }
    println!("{}", total);
}
