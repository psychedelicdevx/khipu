#include <stdio.h>

#define N 1000000

static long long work(long long n) {
    long long a = 0, b = 1;
    for (long long i = 0; i < n; i++) {
        a = a + b;
        b = b + 1;
    }
    return a % 1000;
}

int main(void) {
    printf("%lld\n", work(N));
    return 0;
}
