#include <stdio.h>
#include <sys/socket.h>
#include <unistd.h>

int main() {
    int sv[2];
    if (socketpair(AF_UNIX, SOCK_STREAM, 0, sv) < 0) {
        perror("socketpair");
        return 1;
    }
    
    char buf = 'a';
    int iterations = 100000;
    for (int i = 0; i < iterations; i++) {
        if (write(sv[0], &buf, 1) != 1) {
            return 1;
        }
        if (read(sv[1], &buf, 1) != 1) {
            return 1;
        }
    }
    
    close(sv[0]);
    close(sv[1]);
    return 0;
}
