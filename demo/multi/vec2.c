/* vec2.c — 2D integer vector library implementation */
#include <hack.h>
#include "vec2.h"

int vec2_add_x(int ax, int ay, int bx, int by) {
    return ax + bx;
}

int vec2_add_y(int ax, int ay, int bx, int by) {
    return ay + by;
}

int vec2_scale_x(int x, int y, int s) {
    return x * s;
}

int vec2_scale_y(int x, int y, int s) {
    return y * s;
}

int vec2_dot(int ax, int ay, int bx, int by) {
    return ax * bx + ay * by;
}

int vec2_manhattan(int x, int y) {
    return abs(x) + abs(y);
}
