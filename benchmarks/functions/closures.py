N = 500000


def counter():
    count = 0

    def next():
        nonlocal count
        count = count + 1
        return count

    return next


next_ = counter()
last = 0
for i in range(N):
    last = next_()
print(last)
