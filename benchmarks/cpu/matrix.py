N = 40


def make(n, offset):
    return [[(i * j + offset) % 10 for j in range(n)] for i in range(n)]


a = make(N, 1)
b = make(N, 3)
total = 0
for i in range(N):
    row = a[i]
    for j in range(N):
        s = 0
        for k in range(N):
            s = s + row[k] * b[k][j]
        total = total + s
print(total)
