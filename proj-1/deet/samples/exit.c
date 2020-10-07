#include <stdlib.h>

int main() {
    asm("syscall" :: "a"(60), "D"(0));
}
