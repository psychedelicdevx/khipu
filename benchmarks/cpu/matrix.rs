const N: i64 = 40;

fn make(n: usize, offset: usize) -> Vec<Vec<i64>> {
    (0..n)
        .map(|i| (0..n).map(|j| ((i * j + offset) % 10) as i64).collect())
        .collect()
}

fn main() {
    let n = N as usize;
    let a = make(n, 1);
    let b = make(n, 3);
    let mut total = 0i64;
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0i64;
            for k in 0..n {
                sum += a[i][k] * b[k][j];
            }
            total += sum;
        }
    }
    println!("{}", total);
}
