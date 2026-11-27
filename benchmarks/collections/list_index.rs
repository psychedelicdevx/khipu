const N: i64 = 300000;

fn total(values: &mut [i64], rounds: i64) -> i64 {
    let mut acc = 0i64;
    let mut at = 0i64;
    let n = values.len() as i64;
    while at < rounds {
        acc += values[(at % n) as usize];
        at += 1;
    }
    values[0] += 1;
    acc + values[values.len() - 1] + values[0]
}

fn main() {
    let mut items = [3, 4, 5, 6, 7, 8, 9, 10];
    println!("{}", total(&mut items, N));
}
