# Plan 3: Porting Jack OS Games to C

## Overview

The nand2tetris course ships several games written in Jack: Pong, Snake,
Square, Tetris (community variant), and others. The goal here is to port
them to C, compiling with hack_cc, so they run on the Hack CPU via hack_emu
or on real nand2tetris hardware.

This plan treats the porting work as a **translation project** rather than
a compatibility shim. We do not try to run Jack bytecode; we rewrite the
game logic in C, calling hack_cc builtins instead of Jack OS API calls.

---

## Jack OS API Surface vs hack_cc Equivalents

### Math class

| Jack OS | C equivalent | Status |
|---------|-------------|--------|
| `Math.abs(x)` | `abs(x)` | ❌ not implemented |
| `Math.multiply(x,y)` | `x * y` | ✅ (uses `__mul`) |
| `Math.divide(x,y)` | `x / y` | ✅ (uses `__div`) |
| `Math.sqrt(x)` | `sqrt(x)` | ❌ not implemented |
| `Math.max(a,b)` | `max(a,b)` | ❌ not implemented |
| `Math.min(a,b)` | `min(a,b)` | ❌ not implemented |
| `Math.pow(b,e)` | inline or `pow(b,e)` | ❌ not implemented |

### String class

| Jack OS | C equivalent | Status |
|---------|-------------|--------|
| `String.new(maxLen)` | `malloc(maxLen+1)` | ❌ no malloc |
| `s.dispose()` | `free(s)` | ❌ no malloc |
| `s.length()` | `strlen(s)` | ✅ |
| `s.charAt(i)` | `s[i]` | ✅ |
| `s.setCharAt(i,c)` | `s[i] = c` | ✅ |
| `s.appendChar(c)` | `s[len++] = c` | ✅ (inline) |
| `s.eraseLastChar()` | `s[--len] = 0` | ✅ (inline) |
| `s.intValue()` | `atoi(s)` | ❌ not implemented |
| `s.setInt(n)` | `itoa(n, s, 10)` | ❌ not implemented |
| `String.backSpace()` | `128` | ✅ (constant) |
| `String.doubleQuote()` | `'"'` | ✅ (char literal) |
| `String.newLine()` | `'\n'` | ✅ (char literal) |

### Array class

| Jack OS | C equivalent | Status |
|---------|-------------|--------|
| `Array.new(size)` | `malloc(size * sizeof(int))` | ❌ no malloc |
| `a.dispose()` | `free(a)` | ❌ no malloc |
| `a[i]` | `a[i]` | ✅ |

### Output class (text)

| Jack OS | C equivalent | Status |
|---------|-------------|--------|
| `Output.moveCursor(row,col)` | set global cursor pos | partial |
| `Output.printChar(c)` | `putchar(c)` | ✅ (stdout only) |
| `Output.printString(s)` | `puts(s)` | ✅ |
| `Output.printInt(n)` | `printf("%d", n)` | ❌ no printf/%d |
| `Output.println()` | `putchar('\n')` | ✅ |
| `Output.backSpace()` | cursor management | ❌ |

### Keyboard class

| Jack OS | C equivalent | Status |
|---------|-------------|--------|
| `Keyboard.keyPressed()` | `key_pressed()` | ❌ not implemented |
| `Keyboard.readChar()` | blocking `getchar()` | ❌ not implemented |
| `Keyboard.readLine(prompt)` | custom readline | ❌ not implemented |
| `Keyboard.readInt(prompt)` | readline + atoi | ❌ not implemented |

### Screen class

| Jack OS | C equivalent | Status |
|---------|-------------|--------|
| `Screen.clearScreen()` | `clear_screen()` | ✅ |
| `Screen.setColor(bool)` | set global draw_color | ❌ (always black) |
| `Screen.drawPixel(x,y)` | `draw_pixel(x,y)` | ✅ |
| `Screen.drawLine(x1,y1,x2,y2)` | `draw_line(x1,y1,x2,y2)` | ❌ not implemented |
| `Screen.drawRectangle(x1,y1,x2,y2)` | `draw_rect(x1,y1,x2-x1,y2-y1)` | ❌ not implemented |
| `Screen.drawCircle(cx,cy,r)` | `draw_circle(cx,cy,r)` | ❌ not implemented |

### Memory class

| Jack OS | C equivalent | Status |
|---------|-------------|--------|
| `Memory.peek(addr)` | `*(int*)addr` | ✅ (pointer deref) |
| `Memory.poke(addr,v)` | `*(int*)addr = v` | ✅ (pointer deref) |
| `Memory.alloc(size)` | `malloc(size)` | ❌ no malloc |
| `Memory.deAlloc(obj)` | `free(obj)` | ❌ no malloc |

### Sys class

| Jack OS | C equivalent | Status |
|---------|-------------|--------|
| `Sys.halt()` | infinite loop | ✅ (`while(1){}`) |
| `Sys.error(code)` | print and halt | partial |
| `Sys.wait(ms)` | `sys_wait(ms)` | ❌ not implemented |

---

## Minimum Requirements to Port Any Game

From the table above, the **blocking items** (without these, zero games run):

1. `key_pressed()` — all interactive games need keyboard polling
2. `draw_line()`, `draw_rect()`, `fill_rect()` — visual games need these
3. `malloc()` / `free()` — Jack games create objects dynamically
4. `itoa()` / `printf("%d")` — score displays use printInt

Secondary requirements (needed by most games but can be worked around):

5. `abs()`, `min()`, `max()` — game physics
6. `sys_wait(ms)` — game loop timing
7. `draw_circle()` — needed only for Pong ball; can use fill_rect for prototype
8. `break` / `continue` — clean loop exits in game logic
9. `do-while` — some game loops use this pattern

---

## Porting Strategy: Translation Template

Jack uses objects (classes with fields). C uses structs. The translation
is mechanical:

### Jack class → C struct + functions

```jack
// Jack
class Ball {
    field int x, y;
    field int dx, dy;
    
    constructor Ball new(int ax, int ay) {
        let x = ax;
        let y = ay;
        return this;
    }
    
    method void move() {
        let x = x + dx;
        let y = y + dy;
        do Screen.drawPixel(x, y);
        return;
    }
}
```

```c
// C equivalent
typedef struct {
    int x, y;
    int dx, dy;
} Ball;

Ball *ball_new(int ax, int ay) {
    Ball *b = (Ball *)malloc(sizeof(Ball));
    b->x = ax;
    b->y = ay;
    b->dx = 0;
    b->dy = 0;
    return b;
}

void ball_move(Ball *b) {
    b->x += b->dx;
    b->y += b->dy;
    draw_pixel(b->x, b->y);
}
```

### Jack `do ClassName.method(...)` → C function call

```jack
do Ball.move(ball);    // static dispatch, pass object explicitly
```
```c
ball_move(ball);
```

### Jack `while (~(key = 0))` → C `while (key_pressed())`

```jack
while (~(key = 0)) {
    let key = Keyboard.keyPressed();
}
```
```c
while (key_pressed()) {
    // ...
}
```

### Jack `Math.sqrt(x)` → integer square root

There is no C `sqrt` targeting the Hack platform. Implement as:
```c
int isqrt(int n) {
    int r = 0;
    while ((r+1)*(r+1) <= n) r++;
    return r;
}
```
(For a game, this is only called occasionally; the loop cost is acceptable.)

---

## Game-by-Game Porting Roadmap

### Pong (nand2tetris Chapter 7/8 sample)

**Complexity**: Low  
**Requires**: `draw_rect` (bat), `fill_rect` (ball + erase), `key_pressed`,
  `draw_line` (optional border), `sys_wait`, `itoa` (score)

**Objects**: `Ball`, `Bat`, `PongGame`  
**Key mechanics**: bouncing ball, paddle movement, score tracking

Porting estimate: **4–6 hours** once the runtime blockers are resolved.

**Jack API calls used**:
- `Screen.drawRectangle` → `draw_rect` / `fill_rect`
- `Keyboard.keyPressed` → `key_pressed`
- `Sys.wait` → `sys_wait`
- `Output.printInt` (score) → `printf("%d", score)`
- `Screen.clearScreen` → `clear_screen`

---

### Square Dance (nand2tetris Chapter 9 sample)

**Complexity**: Very low  
**Requires**: `draw_rect`, `fill_rect`, `key_pressed`

The Square game draws a movable square and responds to arrow keys.
No score, no physics, very simple.

**Porting estimate**: **2–3 hours** — good first test case.

---

### Snake

**Complexity**: Medium  
**Requires**: `key_pressed`, `malloc`/`free` (or fixed-size ring buffer),
  `draw_pixel`, `sys_wait`

Snake can be implemented without malloc by using a fixed-size circular
buffer for the body, which is actually how most embedded Snake games work.

**Porting estimate**: **5–8 hours**

---

### Tetris (community)

**Complexity**: High  
**Requires**: full feature set including `draw_rect`, `fill_rect`,
  `key_pressed`, `sys_wait`, `malloc` (or static piece storage), `itoa`

**Porting estimate**: **1–2 days** of work

---

## C Translation Helper: `<jack.h>`

A compatibility header that maps Jack OS API calls to hack_cc equivalents,
making the C port easier to write and review against the original Jack source:

```c
// jack.h — Jack OS compatibility shim for C ports
#ifndef JACK_H
#define JACK_H
#include "hack.h"

// Math
#define Math_abs(x)          abs(x)
#define Math_multiply(x,y)   ((x)*(y))
#define Math_divide(x,y)     ((x)/(y))
#define Math_max(a,b)        max(a,b)
#define Math_min(a,b)        min(a,b)

// Screen
#define Screen_clearScreen()               clear_screen()
#define Screen_drawPixel(x,y)             draw_pixel(x,y)
#define Screen_drawLine(x1,y1,x2,y2)      draw_line(x1,y1,x2,y2)
#define Screen_drawRectangle(x1,y1,x2,y2) draw_rect(x1,y1,(x2)-(x1),(y2)-(y1))
#define Screen_drawCircle(cx,cy,r)        draw_circle(cx,cy,r)

// Keyboard
#define Keyboard_keyPressed()  key_pressed()

// Output (text via screen)
#define Output_printChar(c)    putchar(c)
#define Output_printString(s)  puts(s)
#define Output_printInt(n)     (itoa_buf_fill(n), puts(itoa_buf))
#define Output_println()       putchar('\n')

// Sys
#define Sys_wait(ms)   sys_wait(ms)
#define Sys_halt()     do { while(1){} } while(0)
#define Sys_error(n)   do { printf("ERR %d", n); while(1){} } while(0)

// Memory
#define Memory_alloc(n)    malloc(n)
#define Memory_deAlloc(p)  free(p)
#define Memory_peek(a)     (*(int*)(a))
#define Memory_poke(a,v)   (*(int*)(a) = (v))

// Key codes (nand2tetris values)
#define KEY_UP       131
#define KEY_DOWN     133
#define KEY_LEFT     130
#define KEY_RIGHT    132
#define KEY_ENTER    128
#define KEY_BACKSPACE 129
#define KEY_SPACE    32
#define KEY_ESC      140

#endif
```

With this header, a Jack port can stay very close to the original Jack source
structure, with the differences mostly being syntactic (no `let`, `->` instead
of `.`, explicit `this` as first parameter, etc.).

---

## Emulator Changes for Interactive Gaming

To play these games in `hack_emu`, the emulator needs:

1. **Real-time keyboard input** writing to RAM[24576]:
   - Windows: `ReadConsoleInput` or `GetAsyncKeyState` for non-blocking reads
   - Map nand2tetris key codes (not ASCII for special keys)
   - Key is cleared when released (poll-based, not event-based)

2. **Real-time screen output** — the emulator must render incrementally,
   not just dump a PPM at the end:
   - Option A: render to a terminal using block characters (simple, no deps)
   - Option B: use a GUI library (`minifb`, `pixels`, `softbuffer`)
   - Option C: output PPM frames periodically (for testing, not playing)

3. **Frame rate control**: games call `Sys.wait(ms)`. The emulator needs to
   map this to real wall-clock time. The Hack CPU runs at ~1 MHz nand2tetris
   canonical clock; hack_emu runs much faster, so `sys_wait` must sleep.

4. **`--interactive` mode**: a new flag that enables real-time keyboard and
   screen rendering. The existing `--max-cycles` / `--screen` mode remains
   for testing.

---

## Recommended Porting Order

1. **Square Dance** — simplest game, validates the full toolchain
2. **Pong** — classic, well-understood, exercises most features
3. **Snake** — exercises key_pressed polling + game loop timing
4. **Tetris** — stress test for the entire system

---

## Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| ROM size — complex games may exceed 32K instructions | Profile with hack_emu; the dead-code eliminator helps; split into modules |
| Stack overflow — Jack games use deep call chains | Monitor SP; increase stack size if needed (adjust bootstrap SP init) |
| Speed — some games rely on Jack VM cycle timing | Implement sys_wait; tune the emulator clock multiplier |
| Integer overflow — 16-bit Hack vs Jack's 16-bit int | Same word size; identical overflow behaviour |
| Missing `sqrt` | Implement integer isqrt; Pong doesn't need it; circle draws do |
