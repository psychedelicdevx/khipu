#include <stdio.h>

#define N 300

int main(void) {
    long long total = 0;
    for (long long i = 0; i < N; i++) {
        for (long long j = 0; j < N; j++) {
            total = total + (i * j) % 7;
        }
    }
    printf("%lld\n", total);
    return 0;
}
