/* hello.c — Classic "Hello, World!" demonstrating puts and putchar.
 *
 * This is the simplest possible demo. The linker pulls in __puts and nothing
 * else — showing dead-code elimination at work.
 *
 * Compile: hack_cc hello.c
 * Run:     hack_emu hello.hack
 */
#include <hack.h>

int main() {
    puts("Hello, World!");
    puts("Welcome to the Hack platform.");
    putchar('H');
    putchar('i');
    putchar('!');
    putchar(10);   /* newline */
    return 0;
}
