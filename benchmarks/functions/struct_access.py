N = 1000000


class Point:
    __slots__ = ("x", "y")

    def __init__(self, x, y):
        self.x = x
        self.y = y


def make(x, y):
    return Point(x, y)


def area(p):
    return p.x * p.y


def work(n):
    point = make(3, 4)
    total = 0
    i = 0
    while i < n:
        total = total + area(point)
        i = i + 1
    return total


print(work(N))
