/* vec2.h — 2D integer vector library (header / forward declarations) */
#ifndef VEC2_H
#define VEC2_H

/* Add two vectors, return x or y component. */
int vec2_add_x(int ax, int ay, int bx, int by);
int vec2_add_y(int ax, int ay, int bx, int by);

/* Scale a vector by a scalar, return x or y component. */
int vec2_scale_x(int x, int y, int s);
int vec2_scale_y(int x, int y, int s);

/* Dot product. */
int vec2_dot(int ax, int ay, int bx, int by);

/* Manhattan length |x| + |y|. */
int vec2_manhattan(int x, int y);

#endif
