#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define N 100000

int main(void) {
    char *flags = malloc(N + 1);
    memset(flags, 1, N + 1);
    flags[0] = 0;
    flags[1] = 0;
    for (long long p = 2; p * p <= N; p++) {
        if (flags[p]) {
            for (long long m = p * p; m <= N; m += p) {
                flags[m] = 0;
            }
        }
    }
    long long count = 0;
    for (long long i = 0; i <= N; i++) {
        count += flags[i];
    }
    printf("%lld\n", count);
    free(flags);
    return 0;
}
