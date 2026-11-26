#include <stdio.h>

#define N 1000000

static long long counter = 0;

__attribute__((noinline)) static void bump(void) {
    counter = counter + 1;
}

int main(void) {
    for (long long i = 0; i < N; i++) {
        bump();
    }
    printf("%lld\n", counter);
    return 0;
}
