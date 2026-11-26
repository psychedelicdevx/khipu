#include <stdio.h>
#include <string.h>

#define N 200000

int main(void) {
    long long count = 0;
    char line[64];
    for (long long i = 0; i < N; i++) {
        snprintf(line, sizeof line, "item %lld: ", i);
        strcat(line, "ok");
        count += (long long)strlen(line);
    }
    printf("%lld\n", count);
    return 0;
}
