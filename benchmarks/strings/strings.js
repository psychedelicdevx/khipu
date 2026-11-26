const N = 200000
let count = 0
for (let i = 0; i < N; i++) {
    const line = `item ${i}: ` + "ok"
    count = count + line.length
}
console.log(count)
