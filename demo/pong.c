/*
 * pong.c — Single-player Pong for the Hack platform.
 *
 * Player controls the left paddle with Up/Down arrow keys.
 * The right paddle is a simple AI opponent.
 *
 * Compile:
 *   hack_cc -I include -D HACK_OUTPUT_SCREEN demo/pong.c -o demo/pong.hackem
 * Run:
 *   hack_emu demo/pong.hackem
 */
#define HACK_OUTPUT_SCREEN
#include <hack.h>

/* ── Screen ─────────────────────────────────────────── */
#define SW        512
#define SH        256

/* ── Walls ──────────────────────────────────────────── */
#define WALL_TOP  12      /* y of top wall (2 px: 12-13)    */
#define WALL_BOT  252     /* y of bottom wall (2 px: 252-253) */
#define WALL_H    2
#define PLAY_TOP  14      /* first playable row              */
#define PLAY_BOT  252     /* first row of bottom wall        */

/* ── Ball ───────────────────────────────────────────── */
#define BW        6
#define BH        6

/* ── Paddles ────────────────────────────────────────── */
#define PW        8
#define PH        48
#define P1_X      10
#define P2_X      494     /* SW - PW - 10 */
#define PAD_MIN   14      /* PLAY_TOP */
#define PAD_MAX   204     /* PLAY_BOT - PH */
#define PLR_SPD   3
#define AI_SPD    2

/* ── Keys ───────────────────────────────────────────── */
#define KEY_UP    131
#define KEY_DOWN  133

/* ── Game state ─────────────────────────────────────── */
int bx;
int by;
int bdx;
int bdy;
int p1y;
int p2y;
int s1;
int s2;

void show_score(void) {
    char buf[4];
    clear_rect(192, 0, 128, 11);
    itoa(s1, buf);
    draw_string(26, 0, buf);
    draw_string(29, 0, "-");
    itoa(s2, buf);
    draw_string(31, 0, buf);
}

void new_round(int dir) {
    bx  = 253;   /* SW/2 - BW/2 */
    by  = 125;   /* SH/2 - BH/2 */
    bdx = dir * 2;
    bdy = 1;
}

int main(void) {
    int key;
    int cy;
    int ai_mid;
    int b_mid;

    clear_screen();

    /* Walls */
    fill_rect(0, WALL_TOP, SW, WALL_H);
    fill_rect(0, WALL_BOT, SW, WALL_H);

    /* Centre dashes */
    cy = PLAY_TOP + 4;
    while (cy < WALL_BOT - 8) {
        fill_rect(255, cy, 2, 8);
        cy = cy + 16;
    }

    /* Init */
    p1y = 104;   /* SH/2 - PH/2 */
    p2y = 104;
    s1  = 0;
    s2  = 0;
    new_round(1);

    fill_rect(P1_X, p1y, PW, PH);
    fill_rect(P2_X, p2y, PW, PH);
    show_score();

    /* ── Main loop ──────────────────────────────────── */
    while (1) {

        /* Player 1: Up/Down arrows */
        key = read_key();
        if (key == KEY_UP) {
            if (p1y - PLR_SPD >= PAD_MIN) {
                clear_rect(P1_X, p1y + PH - PLR_SPD, PW, PLR_SPD);
                p1y = p1y - PLR_SPD;
                fill_rect(P1_X, p1y, PW, PLR_SPD);
            }
        } else if (key == KEY_DOWN) {
            if (p1y + PLR_SPD <= PAD_MAX) {
                clear_rect(P1_X, p1y, PW, PLR_SPD);
                p1y = p1y + PLR_SPD;
                fill_rect(P1_X, p1y + PH - PLR_SPD, PW, PLR_SPD);
            }
        }

        /* AI: track ball centre */
        ai_mid = p2y + 24;    /* p2y + PH/2 */
        b_mid  = by  + 3;     /* by  + BH/2 */
        if (ai_mid + 1 < b_mid) {
            if (p2y + AI_SPD <= PAD_MAX) {
                clear_rect(P2_X, p2y, PW, AI_SPD);
                p2y = p2y + AI_SPD;
                fill_rect(P2_X, p2y + PH - AI_SPD, PW, AI_SPD);
            }
        } else if (ai_mid - 1 > b_mid) {
            if (p2y - AI_SPD >= PAD_MIN) {
                clear_rect(P2_X, p2y + PH - AI_SPD, PW, AI_SPD);
                p2y = p2y - AI_SPD;
                fill_rect(P2_X, p2y, PW, AI_SPD);
            }
        }

        /* Erase ball */
        clear_rect(bx, by, BW, BH);

        /* Move ball */
        bx = bx + bdx;
        by = by + bdy;

        /* Wall bounces */
        if (by < PLAY_TOP) {
            bdy = 1;
            by  = PLAY_TOP;
        }
        if (by + BH > PLAY_BOT) {
            bdy = -1;
            by  = PLAY_BOT - BH;
        }

        /* Left paddle bounce */
        if (bdx < 0) {
            if (bx <= P1_X + PW && bx + BW > P1_X) {
                if (by + BH > p1y && by < p1y + PH) {
                    int rel;
                    rel = (by + 3) - (p1y + 24);  /* BH/2=3, PH/2=24 */
                    if (rel < -12) {              /* -PH/4 */
                        bdy = -2;
                    } else if (rel > 12) {        /* PH/4 */
                        bdy = 2;
                    }
                    bdx = 2;
                    bx  = P1_X + PW;
                }
            }
        }

        /* Right paddle bounce */
        if (bdx > 0) {
            if (bx + BW >= P2_X && bx < P2_X + PW) {
                if (by + BH > p2y && by < p2y + PH) {
                    int rel;
                    rel = (by + 3) - (p2y + 24);  /* BH/2=3, PH/2=24 */
                    if (rel < -12) {              /* -PH/4 */
                        bdy = -2;
                    } else if (rel > 12) {        /* PH/4 */
                        bdy = 2;
                    }
                    bdx = -2;
                    bx  = P2_X - BW;
                }
            }
        }

        /* Score: ball off left */
        if (bx + BW < P1_X) {
            s2 = s2 + 1;
            show_score();
            sys_wait(600);
            new_round(-1);
        }

        /* Score: ball off right */
        if (bx > P2_X + PW) {
            s1 = s1 + 1;
            show_score();
            sys_wait(600);
            new_round(1);
        }

        /* Draw ball */
        fill_rect(bx, by, BW, BH);

        sys_wait(30);
    }

    return 0;
}
