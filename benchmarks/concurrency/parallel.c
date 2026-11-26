#include <pthread.h>
#include <stdio.h>

#define N 100000
#define TASKS 8

static long long results[TASKS];

static void *crunch(void *arg) {
    long long seed = (long long)arg;
    long long total = 0;
    for (long long i = 0; i < N; i++) {
        total = (total + i * seed) % 1000003;
    }
    results[seed - 1] = total;
    return NULL;
}

int main(void) {
    pthread_t threads[TASKS];
    for (long long seed = 1; seed <= TASKS; seed++) {
        pthread_create(&threads[seed - 1], NULL, crunch, (void *)seed);
    }
    long long sum = 0;
    for (int t = 0; t < TASKS; t++) {
        pthread_join(threads[t], NULL);
        sum += results[t];
    }
    printf("%lld\n", sum);
    return 0;
}
