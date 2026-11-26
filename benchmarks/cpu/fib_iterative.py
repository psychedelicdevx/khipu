N = 100000


def fib(n):
    a = 0
    b = 1
    i = 0
    while i < n:
        a, b = b, a + b
        i = i + 1
    return a


total = 0
i = 0
while i < N:
    total = (total + fib(i % 50)) % 1000000007
    i = i + 1
print(total)
