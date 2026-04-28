/*
 * square.c — Port of the nand2tetris Chapter 9 "Square Dance" game.
 *
 * A black square starts at the top-left corner of the screen.
 * Controls (same key codes as the original Jack version):
 *   Arrow keys  — move the square (continues while key held)
 *   Q           — quit
 *   Z           — shrink the square by 2 pixels
 *   X           — grow the square by 2 pixels
 *
 * Movement is smooth: only the two-pixel leading/trailing strip is
 * redrawn each step, exactly as in the original Square.jack.
 *
 * Compile:
 *   hack_cc -I include demo/square.c -o demo/square.hackem
 * Run:
 *   hack_emu demo/square.hackem
 */
#define HACK_OUTPUT_SCREEN
#include <hack.h>

/* nand2tetris key codes (uppercase letters + arrow keys) */
#define KEY_UP    131
#define KEY_DOWN  133
#define KEY_LEFT  130
#define KEY_RIGHT 132
#define KEY_Q     81    /* 'Q' — quit          */
#define KEY_Z     90    /* 'Z' — shrink square */
#define KEY_X     88    /* 'X' — grow square   */

/* Screen bounds (matching original Square.jack constraints) */
#define MAX_X     510
#define MAX_Y     254

/* Square state */
int sq_x;
int sq_y;
int sq_size;
int direction; /* 0=none, 1=up, 2=down, 3=left, 4=right */

/* Draw / erase the full square (used on size change). */
void draw_square(void) {
    fill_rect(sq_x, sq_y, sq_size, sq_size);
}

void erase_square(void) {
    clear_rect(sq_x, sq_y, sq_size, sq_size);
}

/*
 * Move by 2 pixels in the current direction.
 * Only the two-pixel trailing edge is erased and the leading edge
 * is drawn, so the bulk of the square stays on screen without flicker.
 */
void move_square(void) {
    if (direction == 1) {                           /* up */
        if (sq_y > 1) {
            clear_rect(sq_x, sq_y + sq_size - 2, sq_size, 2);
            sq_y = sq_y - 2;
            fill_rect(sq_x, sq_y, sq_size, 2);
        }
    } else if (direction == 2) {                    /* down */
        if (sq_y + sq_size < MAX_Y) {
            clear_rect(sq_x, sq_y, sq_size, 2);
            sq_y = sq_y + 2;
            fill_rect(sq_x, sq_y + sq_size - 2, sq_size, 2);
        }
    } else if (direction == 3) {                    /* left */
        if (sq_x > 1) {
            clear_rect(sq_x + sq_size - 2, sq_y, 2, sq_size);
            sq_x = sq_x - 2;
            fill_rect(sq_x, sq_y, 2, sq_size);
        }
    } else if (direction == 4) {                    /* right */
        if (sq_x + sq_size < MAX_X) {
            clear_rect(sq_x, sq_y, 2, sq_size);
            sq_x = sq_x + 2;
            fill_rect(sq_x + sq_size - 2, sq_y, 2, sq_size);
        }
    }
    sys_wait(5);
}

int main(void) {
    int key;
    int exit_game;

    clear_screen();

    sq_x      = 0;
    sq_y      = 0;
    sq_size   = 30;
    direction = 0;
    draw_square();

    exit_game = 0;
    while (!exit_game) {

        /* Wait for a key to be pressed, moving continuously meanwhile. */
        key = 0;
        while (key == 0) {
            key = read_key();
            move_square();
        }

        /* Process the key. */
        if (key == KEY_Q) {
            exit_game = 1;
        } else if (key == KEY_Z) {
            if (sq_size > 2) {
                erase_square();
                sq_size = sq_size - 2;
                draw_square();
            }
        } else if (key == KEY_X) {
            if ((sq_y + sq_size) < MAX_Y && (sq_x + sq_size) < MAX_X) {
                erase_square();
                sq_size = sq_size + 2;
                draw_square();
            }
        } else if (key == KEY_UP) {
            direction = 1;
        } else if (key == KEY_DOWN) {
            direction = 2;
        } else if (key == KEY_LEFT) {
            direction = 3;
        } else if (key == KEY_RIGHT) {
            direction = 4;
        }

        /* Wait for the key to be released, still moving. */
        while (key != 0) {
            key = read_key();
            move_square();
        }
    }

    clear_screen();
    return 0;
}
