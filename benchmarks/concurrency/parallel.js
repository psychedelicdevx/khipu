const N = 100000
const TASKS = 8
const { Worker, isMainThread, parentPort, workerData } = require("worker_threads")

function crunch(seed) {
    let total = 0
    let i = 0
    while (i < N) {
        total = (total + i * seed) % 1000003
        i = i + 1
    }
    return total
}

if (isMainThread) {
    let sum = 0
    let finished = 0
    for (let seed = 1; seed <= TASKS; seed++) {
        const worker = new Worker(__filename, { workerData: seed })
        worker.on("message", (total) => {
            sum += total
            finished += 1
            if (finished === TASKS) console.log(sum)
        })
    }
} else {
    parentPort.postMessage(crunch(workerData))
}
