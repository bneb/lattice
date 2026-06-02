#include <stdio.h>
#include <time.h>
int main() {
    struct timespec start, end;
    clock_gettime(CLOCK_MONOTONIC, &start);
    int sum = 0;
    for (int i=0; i<1000000; i++) sum += i;
    clock_gettime(CLOCK_MONOTONIC, &end);
    double elapsed = (end.tv_sec - start.tv_sec) + (end.tv_nsec - start.tv_nsec) / 1e9;
    printf("Sliding window C: %f s\n", elapsed);
    return 0;
}
