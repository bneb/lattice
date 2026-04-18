#include <stdio.h>
#include <stdint.h>
extern uint32_t user__browser__paint__CMD_COUNT;
void print_cmd_count() {
    // Wait, Salt global variables might not be exported directly. Let's just grep the binary to see if it emitted to standard out!
}
