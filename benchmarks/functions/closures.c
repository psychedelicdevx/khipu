#include <stdio.h>

#define N 500000

struct counter {
    long long count;
};

__attribute__((noinline)) static long long next(struct counter *c) {
    c->count = c->count + 1;
    return c->count;
}

int main(void) {
    struct counter c = {0};
    long long last = 0;
    for (long long i = 0; i < N; i++) {
        last = next(&c);
    }
    printf("%lld\n", last);
    return 0;
}
