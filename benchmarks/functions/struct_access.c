#include <stdio.h>

#define N 1000000

struct point {
    long long x;
    long long y;
};

long long work(long long n) {
    volatile struct point point = {3, 4};
    long long total = 0;
    for (long long i = 0; i < n; i++) {
        total = total + point.x * point.y;
    }
    return total;
}

int main(void) {
    printf("%lld\n", work(N));
    return 0;
}
