const N = 100000

function fib(n) {
    let a = 0
    let b = 1
    for (let i = 0; i < n; i++) {
        const next = a + b
        a = b
        b = next
    }
    return a
}

let total = 0
for (let i = 0; i < N; i++) {
    total = (total + fib(i % 50)) % 1000000007
}
console.log(total)
