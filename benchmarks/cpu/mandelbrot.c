#include <stdio.h>

#define N 60

int main(void) {
    long long inside = 0;
    for (int py = 0; py < N; py++) {
        for (int px = 0; px < N; px++) {
            double cx = px * 3.0 / N - 2.0;
            double cy = py * 2.0 / N - 1.0;
            double x = 0, y = 0;
            int i = 0;
            while (i < 50 && x * x + y * y <= 4) {
                double t = x * x - y * y + cx;
                y = 2 * x * y + cy;
                x = t;
                i = i + 1;
            }
            if (i == 50) inside = inside + 1;
        }
    }
    printf("%lld\n", inside);
    return 0;
}
