#include <stdio.h>

#define N 200000

struct point {
    long long x;
    long long y;
};

static struct point point(long long x, long long y) {
    struct point p = {x, y};
    return p;
}

int main(void) {
    long long total = 0;
    for (long long i = 0; i < N; i++) {
        struct point p = point(i, i + 1);
        total = total + p.x + p.y * 2;
    }
    printf("%lld\n", total);
    return 0;
}
