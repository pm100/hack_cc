/* input.c — Keyboard echo demo.
 *
 * Reads characters from the keyboard and echoes them to the screen.
 * Press Backspace to delete, Enter to start a new line.
 * Type 'Q' to quit.
 *
 * Compile: hack_cc -I include -f hackem demo/input.c -o demo/input.hackem
 * Run:     hackem demo/input.hackem
 */
//#define HACK_OUTPUT_SCREEN
#include <hack.h>

int main() {
    int ch;

    puts("Type something! (Q to quit)");
    puts("");

    while (1) {
        ch = getchar();

        if (ch == 'Q')
            break;

        if (ch == 129) {
            /* Backspace (Hack keycode 129) */
            putchar(8);
        } else if (ch == 128) {
            /* Enter (Hack keycode 128) -> newline */
            putchar(10);
        } else {
            putchar(ch);
        }
    }

    puts("");
    puts("Goodbye!");
    return 0;
}
