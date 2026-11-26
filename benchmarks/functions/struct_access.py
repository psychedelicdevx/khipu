N = 1000000


class Point:
    __slots__ = ("x", "y")

    def __init__(self, x, y):
        self.x = x
        self.y = y


point = Point(3, 4)
total = 0
i = 0
while i < N:
    total = total + point.x * point.y
    i = i + 1
print(total)
