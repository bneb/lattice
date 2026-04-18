#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>
#include <mach/mach_time.h>

// ============================================================================
// Epic 90: POSIX Pipe Bridge for E2E stdout capture
// ============================================================================

void sys_exec_capture_stdout(const char* cmd, unsigned char* out_buf, unsigned int max_len, unsigned int* out_len) {
    FILE* fp = popen(cmd, "r");
    if (!fp) {
        *out_len = 0;
        return;
    }
    unsigned int bytes_read = (unsigned int)fread(out_buf, 1, max_len, fp);
    *out_len = bytes_read;
    pclose(fp);
}

