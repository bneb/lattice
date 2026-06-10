#include <stdio.h>
#include <sys/epoll.h>
#include <stddef.h>
int main() {
    printf("size=%zu offset=%zu\n", sizeof(struct epoll_event), offsetof(struct epoll_event, data));
    return 0;
}
