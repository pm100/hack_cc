/* main.c — Multi-file linking demo.
 *
 * Demonstrates compiling three C files into one program:
 *
 *   hack_cc main.c vec2.c stats.c -o multi_demo.asm
 *
 * main.c uses:
 *   vec2.h / vec2.c  — 2D integer vector arithmetic
 *   stats.h / stats.c — array statistics (sum, avg, min, max, count)
 *   <hack.h>          — runtime library (puts, itoa, abs, ...)
 *
 * Each .c file is compiled independently; the compiler merges the
 * parsed ASTs and runs a single sema + codegen pass, then the
 * symbol-scan linker pulls in only the runtime modules needed.
 */
#include <hack.h>
#include "vec2.h"
#include "stats.h"

/* Print "label: n\n" using the text output port. */
static void print_int(char *label, int n) {
    char line[32];
    char num[12];
    strcpy(line, label);
    strcat(line, ": ");
    itoa(n, num);
    strcat(line, num);
    puts(line);
}

int main() {
    /* ── Vector arithmetic ─────────────────────────────────────── */
    int ax;
    int ay;
    int bx;
    int by;
    int rx;
    int ry;
    int scores[6];
    int i;

    ax = 3;  ay = 4;
    bx = 1;  by = -2;

    puts("=== vec2 demo ===\n");

    rx = vec2_add_x(ax, ay, bx, by);
    ry = vec2_add_y(ax, ay, bx, by);
    print_int("add.x", rx);        /* 4  */
    print_int("add.y", ry);        /* 2  */

    rx = vec2_scale_x(ax, ay, 3);
    ry = vec2_scale_y(ax, ay, 3);
    print_int("scale.x", rx);      /* 9  */
    print_int("scale.y", ry);      /* 12 */

    print_int("dot", vec2_dot(ax, ay, bx, by));          /* 3-8 = -5  */
    print_int("manhattan", vec2_manhattan(-6, 8));        /* 14        */

    /* ── Array statistics ─────────────────────────────────────── */
    puts("=== stats demo ===\n");

    scores[0] = 10;
    scores[1] = 20;
    scores[2] = 30;
    scores[3] = 20;
    scores[4] = 50;
    scores[5] = 20;

    print_int("sum",   stats_sum(scores, 6));             /* 150 */
    print_int("avg",   stats_avg(scores, 6));             /* 25  */
    print_int("min",   stats_min(scores, 6));             /* 10  */
    print_int("max",   stats_max(scores, 6));             /* 50  */
    print_int("count(20)", stats_count(scores, 6, 20));   /* 3   */

    return 0;
}
