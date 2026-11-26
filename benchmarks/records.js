function point(x, y) {
    return { x: x, y: y }
}
let total = 0
for (let i = 0; i < 200000; i++) {
    const p = point(i, i + 1)
    total = total + p.x * p.y
}
console.log(total)
