const N = 300000
const items = []
for (let i = 0; i < N; i++) {
    items.push(i * 2)
}
let total = 0
for (const item of items) {
    total = total + item
}
console.log(total + items[items.length - 1])
