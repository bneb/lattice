#include <stdio.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>

int main() {
    int fd = shm_open("prisimi_test_lldb", O_CREAT | O_RDWR, 0666);
    ftruncate(fd, 131072);
    char fd_str[10];
    snprintf(fd_str, 10, "%d", fd);
    
    printf("Spawning with FD %s\n", fd_str);
    execl("/opt/homebrew/opt/llvm/bin/lldb", "lldb", "--batch", "-o", "run", "-o", "bt", "--", "/tmp/salt_build/prisimi_renderer", "--ipc-fd", fd_str, "--url", "https://google.com", NULL);
    
    // fallback if llvm path is different
    execl("/usr/bin/lldb", "lldb", "--batch", "-o", "run", "-o", "bt", "--", "/tmp/salt_build/prisimi_renderer", "--ipc-fd", fd_str, "--url", "https://google.com", NULL);
    return 0;
}
