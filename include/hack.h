#ifndef __HACK_H__
#define __HACK_H__

/* --- basic I/O --- */
int putchar(int c);
int puts(char *s);
int getchar(void);
int read_key(void);

/* Screen-buffer output: define HACK_OUTPUT_SCREEN to route putchar/puts
 * through the text console instead of the emulator output port (RAM[32767]).
 * Useful when targeting the standard nand2tetris emulator. */
#ifdef HACK_OUTPUT_SCREEN
int putchar_screen(int c);
int puts_screen(const char *s);
#define putchar putchar_screen
#define puts    puts_screen
#endif

/* --- screen (pixels) --- */
void draw_pixel(int x, int y);
void clear_pixel(int x, int y);
void fill_screen(void);
void clear_screen(void);

/* --- text rendering --- */
void draw_char(int col, int row, int c);
void draw_string(int col, int row, char *s);
void print_at(int col, int row, char *s);

/* --- graphics helpers --- */
void draw_line(int x1, int y1, int x2, int y2);
void draw_rect(int x, int y, int w, int h);
void fill_rect(int x, int y, int w, int h);
void clear_rect(int x, int y, int w, int h);

/* --- math --- */
int abs(int x);
int min(int a, int b);
int max(int a, int b);

/* --- string functions --- */
char *strcpy(char *dst, char *src);
int   strcmp(char *a, char *b);
char *strcat(char *dst, char *src);
int   strlen(char *s);
char *strchr(char *s, int c);
char *itoa(int n, char *buf);
int   atoi(char *s);

/* --- memory --- */
void *malloc(int n);
void  free(void *ptr);
void *memset(void *ptr, int val, int n);
void *memcpy(void *dst, void *src, int n);

/* --- system --- */
void sys_wait(int ms);
int  rand(void);
void srand(int seed);

#endif /* __HACK_H__ */
