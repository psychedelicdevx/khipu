#include <stdio.h>

#define N 1000000

struct point {
    long long x;
    long long y;
};

int main(void) {
    volatile struct point point = {3, 4};
    long long total = 0;
    for (long long i = 0; i < N; i++) {
        total = total + point.x * point.y;
    }
    printf("%lld\n", total);
    return 0;
}
