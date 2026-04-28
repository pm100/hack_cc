/*
 * breakout.c — Port of the nand2tetris Chapter 9 Pong game.
 *
 * Single-player bat-and-ball:
 *   - Ball bounces off the top wall and both side walls.
 *   - Player controls the bat at the bottom with LEFT / RIGHT arrow keys.
 *   - Each successful bat hit scores a point; every 5 hits the ball speeds up.
 *   - Miss the ball and it's Game Over; press any key to restart.
 *
 * Compile:
 *   hack_cc -I include demo/breakout.c -o demo/breakout.hackem
 * Run:
 *   hack_emu demo/breakout.hackem
 */
#define HACK_OUTPUT_SCREEN
#include <hack.h>

/* ── Screen ──────────────────────────────────────────── */
#define SW          512
#define SH          256

/* ── Top wall ────────────────────────────────────────── */
#define WALL_Y      10
#define WALL_H      3
#define PLAY_TOP    (WALL_Y + WALL_H)   /* 13 */

/* ── Bat ─────────────────────────────────────────────── */
#define BAT_W       80
#define BAT_H       6
#define BAT_Y       240
#define BAT_MIN     0
#define BAT_MAX     (SW - BAT_W)        /* 432 */
#define BAT_SPD     5

/* ── Ball ────────────────────────────────────────────── */
#define BALL_SZ     6

/* ── Keys ────────────────────────────────────────────── */
#define KEY_LEFT    130
#define KEY_RIGHT   132

/* ── Speed schedule ──────────────────────────────────── */
#define DELAY_START 40      /* ms per frame at start */
#define DELAY_MIN   10      /* fastest the game gets  */
#define SPEED_STEP  5       /* hits between speed-ups */

/* ── Game state (globals) ────────────────────────────── */
int bx;
int by;
int bdx;
int bdy;
int batx;
int score;
int delay;

/* ── Helpers ─────────────────────────────────────────── */

void draw_score(void) {
    char buf[6];
    clear_rect(210, 0, 90, WALL_Y);
    itoa(score, buf);
    draw_string(27, 0, buf);
}

void init_game(void) {
    clear_screen();

    /* Top wall */
    fill_rect(0, WALL_Y, SW, WALL_H);

    /* Bat */
    batx = (SW - BAT_W) / 2;
    fill_rect(batx, BAT_Y, BAT_W, BAT_H);

    /* Ball — start just below the wall, moving down-right */
    bx  = SW / 2 - BALL_SZ / 2;
    by  = PLAY_TOP + 10;
    bdx = 2;
    bdy = 2;

    score = 0;
    delay = DELAY_START;

    draw_score();
}

/* ── Main ────────────────────────────────────────────── */

int main(void) {
    int key;
    int new_batx;
    int game_over;

restart:
    init_game();
    game_over = 0;

    while (!game_over) {

        /* ── Input ── */
        key = read_key();

        if (key == KEY_LEFT) {
            new_batx = batx - BAT_SPD;
            if (new_batx < BAT_MIN) new_batx = BAT_MIN;
            if (new_batx != batx) {
                clear_rect(batx + BAT_W - BAT_SPD, BAT_Y, BAT_SPD, BAT_H);
                batx = new_batx;
                fill_rect(batx, BAT_Y, BAT_SPD, BAT_H);
            }
        } else if (key == KEY_RIGHT) {
            new_batx = batx + BAT_SPD;
            if (new_batx > BAT_MAX) new_batx = BAT_MAX;
            if (new_batx != batx) {
                clear_rect(batx, BAT_Y, BAT_SPD, BAT_H);
                batx = new_batx;
                fill_rect(batx + BAT_W - BAT_SPD, BAT_Y, BAT_SPD, BAT_H);
            }
        }

        /* ── Erase ball ── */
        clear_rect(bx, by, BALL_SZ, BALL_SZ);

        /* ── Move ball ── */
        bx = bx + bdx;
        by = by + bdy;

        /* ── Wall bounces ── */

        /* Top wall */
        if (by < PLAY_TOP) {
            bdy = 2;
            by  = PLAY_TOP;
        }

        /* Left wall */
        if (bx < 0) {
            bdx = 2;
            bx  = 0;
        }

        /* Right wall */
        if (bx + BALL_SZ > SW) {
            bdx = -2;
            bx  = SW - BALL_SZ;
        }

        /* ── Bat collision ── */
        if (bdy > 0) {
            if (by + BALL_SZ >= BAT_Y && by < BAT_Y + BAT_H) {
                if (bx + BALL_SZ > batx && bx < batx + BAT_W) {
                    bdy = -2;
                    by  = BAT_Y - BALL_SZ;

                    score = score + 1;
                    draw_score();

                    /* Speed up every SPEED_STEP hits */
                    if (score % SPEED_STEP == 0) {
                        delay = delay - 5;
                        if (delay < DELAY_MIN) delay = DELAY_MIN;
                    }
                }
            }
        }

        /* ── Miss ── */
        if (by > BAT_Y + BAT_H) {
            game_over = 1;
        }

        if (!game_over) {
            /* ── Draw ball ── */
            fill_rect(bx, by, BALL_SZ, BALL_SZ);
            sys_wait(delay);
        }
    }

    /* ── Game Over screen ── */
    clear_screen();
    draw_string(17, 10, "GAME OVER");
    draw_string(15, 12, "Score:");
    {
        char buf[6];
        itoa(score, buf);
        draw_string(22, 12, buf);
    }
    draw_string(12, 15, "Press any key...");

    /* Wait for a key press then restart */
    while (read_key() == 0) {}
    goto restart;

    return 0;
}
