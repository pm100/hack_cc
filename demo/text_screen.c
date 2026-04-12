/* text_screen.c — Text rendering demo.
 *
 * Draws text using the built-in 8x11 pixel font (64 cols x 23 rows).
 * Exercises: clear_screen, draw_string, draw_char.
 *
 * Compile: hack_cc text_screen.c
 * Run:     hack_emu --screen text_screen.ppm text_screen.hack
 */
#include <hack.h>

int main() {
    clear_screen();

    draw_string(0,  0,  "Hack Platform");
    draw_string(0,  1,  "=============");
    draw_string(0,  3,  "Font: 8x11 pixels");
    draw_string(0,  4,  "Grid: 64 cols x 23 rows");
    draw_string(0,  6,  "Hello from hack_cc!");
    draw_string(0,  8,  "ABCDEFGHIJKLMNOPQRSTUVWXYZ");
    draw_string(0,  9,  "abcdefghijklmnopqrstuvwxyz");
    draw_string(0, 10,  "0123456789 !\"#$%&'()*+,-./:;<=>?@");
    draw_string(0, 12,  "The quick brown fox");
    draw_string(0, 13,  "jumps over the lazy dog.");

    return 0;
}
