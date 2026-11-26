#include <stdio.h>

#define N 100000

static long long fib(long long n) {
    long long a = 0, b = 1;
    for (long long i = 0; i < n; i++) {
        long long next = a + b;
        a = b;
        b = next;
    }
    return a;
}

int main(void) {
    long long total = 0;
    for (long long i = 0; i < N; i++) {
        total = (total + fib(i % 50)) % 1000000007;
    }
    printf("%lld\n", total);
    return 0;
}
