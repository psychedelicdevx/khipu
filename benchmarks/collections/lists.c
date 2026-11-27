#include <stdio.h>

#define N 300000

long long total(long long *values, size_t length, long long rounds) {
    long long acc = 0;
    for (long long at = 0; at < rounds; at++) {
        acc += values[at % (long long)length];
    }
    values[0] = values[0] + 1;
    return acc + values[length - 1] + values[0];
}

int main(void) {
    long long items[] = {3, 4, 5, 6, 7, 8, 9, 10};
    printf("%lld\n", total(items, sizeof items / sizeof *items, N));
    return 0;
}
