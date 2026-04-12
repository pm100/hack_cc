/* graphics.c — Screen graphics demo.
 *
 * Draws a border, fills some rectangles, and draws diagonal lines.
 * Exercises: clear_screen, draw_pixel, draw_line, draw_rect, fill_rect.
 * The linker pulls in __draw_pixel, draw_line, draw_rect, fill_rect,
 * __clear_screen — and nothing else.
 *
 * Compile: hack_cc graphics.c
 * Run:     hack_emu --screen graphics.ppm graphics.hack
 */
#include <hack.h>

int main() {
    /* Clear screen to white */
    clear_screen();

    /* Draw a border around the whole screen (512x256) */
    draw_rect(0, 0, 512, 256);

    /* Fill a solid block in the top-left */
    fill_rect(10, 10, 80, 60);

    /* Fill a solid block in the top-right */
    fill_rect(422, 10, 80, 60);

    /* Draw diagonal lines forming an X in the centre */
    draw_line(100, 80, 400, 180);
    draw_line(400, 80, 100, 180);

    /* Draw a few individual pixels to show draw_pixel */
    draw_pixel(256, 128);
    draw_pixel(255, 128);
    draw_pixel(257, 128);
    draw_pixel(256, 127);
    draw_pixel(256, 129);

    /* Small filled square in the centre */
    fill_rect(236, 108, 40, 40);

    return 0;
}
