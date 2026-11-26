N = 300000


def identity(x):
    return x


total = 0
for i in range(N):
    total = total + identity(i)
print(total)
