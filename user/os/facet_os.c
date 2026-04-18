#include <fcntl.h>
#include <stdint.h>
#include <sys/mman.h>
#include <unistd.h>

#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>

int32_t facet_os_fork() { return fork(); }

void *facet_os_mmap_shared(int32_t size) {
  return mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS,
              -1, 0);
}

int32_t facet_os_create_shm(const char *name, int32_t size) {
  int fd = shm_open(name, O_CREAT | O_RDWR, 0666);
  if (fd >= 0) {
    ftruncate(fd, size);
  }
  return fd;
}

int32_t facet_os_shm_open(const char *name) {
  return shm_open(name, O_RDWR, 0666);
}

uint32_t sys_atomic_load_u32(uint64_t ptr) {
  return __atomic_load_n((uint32_t *)ptr, __ATOMIC_SEQ_CST);
}

void sys_atomic_store_u32(uint64_t ptr, uint32_t val) {
  __atomic_store_n((uint32_t *)ptr, val, __ATOMIC_SEQ_CST);
}

void *facet_os_mmap_fd(int32_t fd, int32_t size) {
  return mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
}

void facet_os_exit(int32_t code) { exit(code); }

void facet_os_usleep(int32_t microseconds) { usleep(microseconds); }

int32_t facet_os_spawn(const char *path) {
  pid_t pid = fork();
  if (pid == 0) {
    char *args[] = {(char *)path, NULL};
    execv(path, args);
    exit(1);
  }
  return (int32_t)pid;
}

void facet_os_kill(int32_t pid) {
  kill(pid, SIGTERM);
  waitpid(pid, NULL, 0);
}

extern int32_t salt_get_argc();
extern const char *salt_get_argv(int32_t idx);

__attribute__((weak)) int32_t ext_get_ipc_fd() {
  int32_t argc = salt_get_argc();
  for (int32_t i = 0; i < argc - 1; i++) {
    const char *arg = salt_get_argv(i);
    if (arg && strcmp(arg, "--ipc-fd") == 0) {
      return atoi(salt_get_argv(i + 1));
    }
  }
  return -1;
}
