import sys

with open('salt-front/runtime.c', 'r') as f:
    content = f.read()

new_funcs = """
#include <fcntl.h>
#include <unistd.h>
#include <sys/mman.h>

int64_t salt_open(const char *path, int flags) {
    return (int64_t)open(path, flags, 0666);
}

int64_t salt_read(int64_t fd, void *buf, uint64_t size) {
    return (int64_t)read((int)fd, buf, (size_t)size);
}

int64_t salt_write(int64_t fd, const void *buf, uint64_t size) {
    return (int64_t)write((int)fd, buf, (size_t)size);
}

int32_t salt_close(int64_t fd) {
    return (int32_t)close((int)fd);
}

uint64_t salt_mmap(int64_t fd, uint64_t size, uint64_t offset) {
    void *ptr = mmap(NULL, (size_t)size, PROT_READ | PROT_WRITE, MAP_SHARED, (int)fd, (off_t)offset);
    if (ptr == MAP_FAILED) {
        return 0;
    }
    return (uint64_t)ptr;
}

int32_t salt_munmap(uint64_t ptr, uint64_t size) {
    return (int32_t)munmap((void*)ptr, (size_t)size);
}

"""

if "salt_open" not in content:
    content = content.replace("int64_t salt_opendir(const char *path) {", new_funcs + "\nint64_t salt_opendir(const char *path) {")

with open('salt-front/runtime.c', 'w') as f:
    f.write(content)
