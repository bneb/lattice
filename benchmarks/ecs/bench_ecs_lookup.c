#include <stdio.h>
#include <unistd.h>

int main() {
    int iterations = 1000000;
    for (int i = 0; i < iterations; i++) {
        pid_t pid = getpid();
        if (pid <= 0) {
            return 1;
        }
    }
    return 0;
}
