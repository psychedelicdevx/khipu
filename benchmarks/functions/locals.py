N = 1000000


def work(n):
    a = 0
    b = 1
    i = 0
    while i < n:
        a = a + b
        b = b + 1
        i = i + 1
    return a % 1000


print(work(N))
