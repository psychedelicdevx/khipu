#include <stdio.h>

#define N 300000

__attribute__((noinline)) static long long identity(long long x) {
    return x;
}

int main(void) {
    long long total = 0;
    for (long long i = 0; i < N; i++) {
        total = total + identity(i);
    }
    printf("%lld\n", total);
    return 0;
}
