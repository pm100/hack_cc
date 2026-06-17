# hack_cc Runtime Library

The runtime library lives in the `lib/` directory. Include `<hack.h>` to get all declarations.

```c
#include <hack.h>
```

Every function is implemented as a self-contained Hack assembly file. Only the functions your program actually calls are linked into the output — unused library code is never emitted.

---

## I/O (`lib/io/`)

### Character and string output

```c
int putchar(int c);
```
Writes one character to the emulator output port (RAM[32767]). Returns `c`. Use `-D HACK_OUTPUT_SCREEN` to redirect to the on-screen text console instead.

```c
int puts(char *s);
```
Writes a null-terminated string followed by a newline. Returns 0.

```c
int getchar(void);
```
Waits for a key press and returns its ASCII value (or Hack keycode for special keys). See key codes below.

```c
int read_key(void);
```
Returns the current value of the keyboard register (RAM[24576]) without waiting. Returns 0 if no key is pressed.

**Screen output mode:** Defining `HACK_OUTPUT_SCREEN` before including `<hack.h>` redefines `putchar` and `puts` to write to the on-screen text console (useful with the nand2tetris CPU Emulator, which does not implement the output port).

### String functions

```c
int   strlen(char *s);
char *strcpy(char *dst, char *src);
char *strcat(char *dst, char *src);
int   strcmp(char *a, char *b);
char *strchr(char *s, int c);
char *itoa(int n, char *buf);   // writes decimal representation of n into buf; returns buf
int   atoi(char *s);            // parses decimal integer from s; returns value
```

---

## Screen — pixels (`lib/screen/`)

The Hack screen is 512×256 pixels. Pixel (0,0) is top-left. Each word of screen memory controls 16 horizontal pixels; bit 0 is the leftmost pixel in the word.

```c
void draw_pixel(int x, int y);    // set pixel (x,y) black
void clear_pixel(int x, int y);   // set pixel (x,y) white
void fill_screen(void);           // fill entire screen black
void clear_screen(void);          // clear entire screen white
```

---

## Screen — shapes (`lib/screen/`)

```c
void draw_line(int x1, int y1, int x2, int y2);
void draw_rect(int x, int y, int w, int h);    // outline only
void fill_rect(int x, int y, int w, int h);    // filled black
void clear_rect(int x, int y, int w, int h);   // filled white
```

All coordinates are in pixels.

---

## Screen — text (`lib/screen/`)

Text is rendered using an embedded 8×11 bitmap font (96 printable ASCII characters, 32–127). The font table occupies 1056 words at RAM[15328] and is only emitted when `draw_char` or `draw_string` is used.

The text grid is 64 columns × 23 rows (512 / 8 = 64 columns, 256 / 11 = 23 rows).

```c
void draw_char(int col, int row, int c);         // draw ASCII character c at grid position (col, row)
void draw_string(int col, int row, char *s);     // draw null-terminated string starting at (col, row)
void print_at(int col, int row, char *s);        // alias for draw_string
```

---

## Math (`lib/math/`, `lib/misc/`)

```c
int abs(int x);
int min(int a, int b);
int max(int a, int b);
```

Multiplication, division, and modulo (`*`, `/`, `%`) are not native Hack instructions. The compiler emits calls to internal helpers automatically:

- `__mul` — 16-bit signed multiply
- `__div` — 16-bit signed divide and modulo
- `__lmul`, `__ldiv`, `__ladd`, `__lsub`, `__lneg`, `__lshl`, `__lshr` — 32-bit (`long`) arithmetic

These are used transparently; you do not call them directly.

---

## Memory (`lib/memory/`)

A simple heap allocator is included. The heap starts just above the global variable area and grows upward.

```c
void *malloc(int n);       // allocate n words; returns pointer or 0 on failure
void  free(void *ptr);     // free a previously allocated block
void *memset(void *ptr, int val, int n);   // fill n words with val; returns ptr
void *memcpy(void *dst, void *src, int n); // copy n words from src to dst; returns dst
```

Note: sizes are in **Hack words** (16 bits), not bytes.

---

## System (`lib/sys/`)

```c
void sys_wait(int ms);   // busy-wait for approximately ms milliseconds
int  rand(void);         // pseudo-random number (LCG); range 0..32767
void srand(int seed);    // seed the random number generator
```

`rand` uses the recurrence `seed = seed * 25173 + 13849` and returns the low 15 bits.

---

## Keyboard key codes

Special keys return values outside the printable ASCII range:

| Key | Code |
|-----|------|
| Enter | 128 |
| Backspace | 129 |
| Left arrow | 130 |
| Up arrow | 131 |
| Right arrow | 132 |
| Down arrow | 133 |

---

## Adding a new library function

1. Create a `.s` file in the appropriate subdirectory of `lib/` (e.g. `lib/io/myfunc.s`).
2. Put `// PROVIDES: myfunc` as the **first line** — this is how the linker discovers it.
3. Add a declaration to `include/hack.h`.

No changes to Rust code are required. The linker will automatically pull in the new file whenever your C code calls `myfunc`.


