#include <stdio.h>

#define N 3000000

int main(void) {
    long long total = 0;
    for (long long i = 0; i < N; i++) {
        total = total + i;
    }
    printf("%lld\n", total);
    return 0;
}
