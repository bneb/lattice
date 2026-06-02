#include <stdio.h>
#include <sched.h>

int main() {
    int iterations = 100000;
    for (int i = 0; i < iterations; i++) {
        sched_yield();
    }
    return 0;
}
