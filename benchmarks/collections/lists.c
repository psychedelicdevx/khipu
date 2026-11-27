#include <stdio.h>
#include <stdlib.h>

#define N 300000

int main(void) {
    size_t capacity = 8, length = 0;
    long long *items = malloc(capacity * sizeof *items);
    for (long long i = 0; i < N; i++) {
        if (length == capacity) {
            capacity *= 2;
            items = realloc(items, capacity * sizeof *items);
        }
        items[length++] = i * 2;
    }
    long long total = 0;
    for (size_t i = 0; i < length; i++) {
        total += items[i];
    }
    printf("%lld\n", total + items[length - 1]);
    free(items);
    return 0;
}
