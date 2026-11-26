from concurrent.futures import ProcessPoolExecutor

N = 100000
TASKS = 8


def crunch(seed):
    total = 0
    i = 0
    while i < N:
        total = (total + i * seed) % 1000003
        i = i + 1
    return total


if __name__ == "__main__":
    with ProcessPoolExecutor(TASKS) as pool:
        print(sum(pool.map(crunch, range(1, TASKS + 1))))
