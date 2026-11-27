const N = 1000000

class Point {
    constructor(x, y) {
        this.x = x
        this.y = y
    }
}

function make(x, y) {
    return new Point(x, y)
}

function area(p) {
    return p.x * p.y
}

function work(n) {
    const point = make(3, 4)
    let total = 0
    let i = 0
    while (i < n) {
        total = total + area(point)
        i = i + 1
    }
    return total
}

console.log(work(N))
