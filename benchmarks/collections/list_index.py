N = 300000

items = [3, 4, 5, 6, 7, 8, 9, 10]


def total(values, rounds):
    acc = 0
    at = 0
    n = len(values)
    while at < rounds:
        acc = acc + values[at % n]
        at = at + 1
    values[0] = values[0] + 1
    return acc + values[len(values) - 1] + values[0]


print(total(items, N))
