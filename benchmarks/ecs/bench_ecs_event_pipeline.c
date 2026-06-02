#include <stdio.h>
#include <unistd.h>
#include <sys/select.h>
#include <sched.h>

int main() {
    int fd[2];
    if (pipe(fd) < 0) {
        perror("pipe");
        return 1;
    }
    
    char buf = 'a';
    int iterations = 50000;
    
    for (int i = 0; i < iterations; i++) {
        // 1. Simulating packet arrival (write to pipe)
        if (write(fd[1], &buf, 1) != 1) {
            return 1;
        }
        
        // 2. Select/poll to detect availability
        fd_set read_fds;
        FD_ZERO(&read_fds);
        FD_SET(fd[0], &read_fds);
        struct timeval timeout = {0, 0}; // non-blocking poll
        int ret = select(fd[0] + 1, &read_fds, NULL, NULL, &timeout);
        if (ret < 0) {
            return 1;
        }
        
        // 3. Read/pop event
        if (ret > 0 && FD_ISSET(fd[0], &read_fds)) {
            if (read(fd[0], &buf, 1) != 1) {
                return 1;
            }
        }
        
        // 4. Yield to simulate scheduling
        sched_yield();
    }
    
    close(fd[0]);
    close(fd[1]);
    return 0;
}
