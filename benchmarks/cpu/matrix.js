const N = 40

function make(n, offset) {
    const rows = []
    for (let i = 0; i < n; i++) {
        const row = []
        for (let j = 0; j < n; j++) {
            row.push((i * j + offset) % 10)
        }
        rows.push(row)
    }
    return rows
}

const a = make(N, 1)
const b = make(N, 3)
let total = 0
for (let i = 0; i < N; i++) {
    const row = a[i]
    for (let j = 0; j < N; j++) {
        let sum = 0
        for (let k = 0; k < N; k++) {
            sum = sum + row[k] * b[k][j]
        }
        total = total + sum
    }
}
console.log(total)
