/* strings.c — String function demo.
 *
 * Exercises: strcpy, strcat, strcmp, strlen, itoa.
 * The linker pulls in __strcpy, __strcat, __strcmp, __strlen, __itoa, __div.
 * __div is included transitively because __itoa calls it.
 *
 * Compile: hack_cc strings.c
 * Run:     hack_emu strings.hack
 */
#include <hack.h>

int main() {
    char buf[64];
    char num_buf[12];
    int n;

    /* strcpy and strcat */
    strcpy(buf, "Hello");
    strcat(buf, ", ");
    strcat(buf, "World");
    strcat(buf, "!");
    puts(buf);                  /* Hello, World! */

    /* strlen */
    n = strlen(buf);
    puts("Length of buffer: ");
    itoa(n, num_buf);
    puts(num_buf);              /* 13 */

    /* strcmp */
    if (strcmp(buf, "Hello, World!") == 0) {
        puts("strcmp: strings match");
    } else {
        puts("strcmp: MISMATCH (bug!)");
    }

    if (strcmp("abc", "abd") < 0) {
        puts("strcmp: abc < abd: correct");
    }

    /* itoa with various values */
    itoa(0, num_buf);   puts(num_buf);
    itoa(42, num_buf);  puts(num_buf);
    itoa(-99, num_buf); puts(num_buf);
    itoa(32767, num_buf); puts(num_buf);

    return 0;
}
