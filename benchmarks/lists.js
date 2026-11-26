const items = []
for (let i = 0; i < 300000; i++) {
    items.push(i * 2)
}
let total = 0
for (const item of items) {
    total = total + item
}
console.log(total + items[items.length - 1])
