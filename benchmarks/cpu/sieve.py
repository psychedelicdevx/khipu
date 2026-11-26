N = 100000
flags = [True] * (N + 1)
flags[0] = False
flags[1] = False
p = 2
while p * p <= N:
    if flags[p]:
        m = p * p
        while m <= N:
            flags[m] = False
            m = m + p
    p = p + 1
count = 0
for flag in flags:
    if flag:
        count = count + 1
print(count)
