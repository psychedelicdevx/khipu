#include <stdio.h>

#define N 1000000

struct point {
    long long x;
    long long y;
};

struct point make(long long x, long long y) {
    volatile struct point p = {x, y};
    return p;
}

long long area(struct point p) {
    return p.x * p.y;
}

long long work(long long n) {
    volatile struct point point = make(3, 4);
    long long total = 0;
    for (long long i = 0; i < n; i++) {
        total = total + area(point);
    }
    return total;
}

int main(void) {
    printf("%lld\n", work(N));
    return 0;
}
