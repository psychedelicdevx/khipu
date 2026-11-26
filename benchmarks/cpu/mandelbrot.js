const N = 60
let inside = 0
for (let py = 0; py < N; py++) {
    for (let px = 0; px < N; px++) {
        const cx = px * 3.0 / N - 2.0
        const cy = py * 2.0 / N - 1.0
        let x = 0
        let y = 0
        let i = 0
        while (i < 50 && x * x + y * y <= 4) {
            const t = x * x - y * y + cx
            y = 2 * x * y + cy
            x = t
            i = i + 1
        }
        if (i === 50) inside = inside + 1
    }
}
console.log(inside)
