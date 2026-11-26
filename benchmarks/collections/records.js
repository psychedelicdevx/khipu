const N = 200000

function point(x, y) {
    return { x: x, y: y }
}

let total = 0
for (let i = 0; i < N; i++) {
    const p = point(i, i + 1)
    total = total + p.x + p.y * 2
}
console.log(total)
