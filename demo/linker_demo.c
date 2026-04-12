/* linker_demo.c — Demonstrates the symbol-scan linker.
 *
 * This program uses features from multiple runtime modules. The linker
 * automatically pulls in ONLY what is needed:
 *
 *   puts         → __puts
 *   itoa         → __itoa  → __div (transitive)
 *   strcpy       → __strcpy
 *   strlen       → __strlen
 *   abs/min/max  → inline (no subroutine)
 *   *            → __mul
 *   draw_string  → __draw_string → __draw_char (transitive)
 *   clear_screen → __clear_screen
 *
 * NOT pulled in (dead code eliminated):
 *   __draw_pixel, __fill_screen, draw_line, draw_rect, fill_rect,
 *   __alloc, malloc, free, __sys_wait, __strcat, __strcmp
 *
 * Compile: hack_cc linker_demo.c
 * Run:     hack_emu --screen linker_demo.ppm linker_demo.hack
 */
#include <hack.h>

int main() {
    char num[12];

    clear_screen();
    draw_string(0, 0, "Linker Demo");
    draw_string(0, 1, "===========");

    /* Math — pulls in __mul and __div (via itoa) */
    itoa(6 * 7, num);
    draw_string(0, 3, "6 * 7 = ");
    draw_string(9, 3, num);

    itoa(abs(-256), num);
    draw_string(0, 4, "abs(-256) = ");
    draw_string(13, 4, num);

    itoa(min(99, 42), num);
    draw_string(0, 5, "min(99,42) = ");
    draw_string(14, 5, num);

    /* String functions — pulls in __strcpy, __strlen */
    itoa(strlen("hello"), num);
    draw_string(0, 6, "strlen(hello) = ");
    draw_string(17, 6, num);

    strcpy(num, "copied!");
    draw_string(0, 7, num);

    draw_string(0, 9,  "Only needed modules");
    draw_string(0, 10, "are linked in.");

    return 0;
}
