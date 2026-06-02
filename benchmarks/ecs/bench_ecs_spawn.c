#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/wait.h>

int main() {
    int iterations = 1000;
    for (int i = 0; i < iterations; i++) {
        pid_t pid = fork();
        if (pid < 0) {
            perror("fork failed");
            exit(1);
        } else if (pid == 0) {
            exit(0);
        } else {
            int status;
            waitpid(pid, &status, 0);
        }
    }
    return 0;
}
