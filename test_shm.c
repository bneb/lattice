#include <stdio.h>
#include <fcntl.h>
#include <sys/mman.h>

int main() {
    int fd = shm_open("/prisimi_cdm_arena", O_RDWR, 0666);
    printf("fd: %d\n", fd);
    return 0;
}
