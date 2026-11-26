#include <stdio.h>

#define N 40

static long long a[N][N];
static long long b[N][N];

int main(void) {
    for (long long i = 0; i < N; i++) {
        for (long long j = 0; j < N; j++) {
            a[i][j] = (i * j + 1) % 10;
            b[i][j] = (i * j + 3) % 10;
        }
    }
    long long total = 0;
    for (long long i = 0; i < N; i++) {
        for (long long j = 0; j < N; j++) {
            long long sum = 0;
            for (long long k = 0; k < N; k++) {
                sum += a[i][k] * b[k][j];
            }
            total += sum;
        }
    }
    printf("%lld\n", total);
    return 0;
}
