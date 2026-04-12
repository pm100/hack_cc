# Plan 2: Runtime Library Design

## Goal

Provide a runtime library that:
1. Covers the full nand2tetris Jack OS API surface (and more)
2. Only includes code that is actually used — dead code must not appear
   in the final binary (ROM is 32K instructions, it fills up fast)
3. Is easy to maintain, test, and extend
4. Can be written in C (compiled by hack_cc itself), assembly, or a mixture

---

## Current Situation

Today, runtime code is embedded directly in `codegen.rs` as Rust strings.
Each builtin is gated with a boolean (`need_mul`, `need_draw_char`, etc.)
and the subroutine is only emitted if the corresponding flag is set.

This works but has problems:
- Builtins must be implemented in Rust string-emit style (ugly, hard to
  read, no syntax highlighting, hard to test)
- Adding a new builtin requires changes in 4 places: `BuiltinKind`, the
  `gen_call` match arm, `scan_builtins_expr`, and `emit_runtime`
- No way to write runtime code in C and have it compiled the same way
- No composability: if `draw_string` needs `draw_char`, it is tracked
  manually via `used.insert(DrawChar)` in the scanner

---

## Proposed Design: `hack.lib` — A Linkable Object Library

### Core Idea

The runtime is stored as a **library of annotated assembly functions** —
plain text `.s` files. The linker (which is just the assembler pass) pulls
in only the functions that are reachable from `main`.

This is analogous to how C's `libc.a` works: each function lives in a
separate translation unit; only referenced ones are linked.

### File Layout

```
src/
  runtime/
    math/
      __mul.s
      __div.s
      __abs.s
      __min.s
      __max.s
    io/
      __putchar.s
      __puts.s
      __strlen.s
      __strcpy.s
      __strcmp.s
      __strcat.s
      __itoa.s
      __printf.s        (depends on __itoa, __puts, __putchar)
    screen/
      __draw_pixel.s
      __clear_pixel.s
      __fill_screen.s
      __clear_screen.s
      __draw_line.s     (depends on __draw_pixel)
      __draw_rect.s     (depends on __draw_pixel or screen-word writes)
      __fill_rect.s
      __draw_char.s     (depends on nothing — direct screen writes)
      __draw_string.s   (depends on __draw_char)
    keyboard/
      __key_pressed.s
      __wait_key.s
    memory/
      __alloc.s         (bump allocator — depends on __heap_ptr global)
      __dealloc.s       (no-op for bump allocator; or free-list later)
    sys/
      __sys_wait.s      (busy-wait loop for timing)
    c/
      hack_math.c       (abs, min, max implemented in C using builtins)
      hack_string.c     (strcpy, strcmp, strcat in C — simpler to write)
      hack_printf.c     (printf in C calling __putchar/__itoa)
  runtime.toml          (manifest: declares each module and its dependencies)
```

### `runtime.toml` Manifest

Each module entry declares:
- `name`: the symbol it defines (e.g. `__mul`)
- `file`: the source file
- `deps`: symbols it calls (controls transitive inclusion)
- `provides_builtins`: which C-level builtin names map to this entry point

```toml
[[module]]
name = "__mul"
file = "math/__mul.s"
deps = []
provides = ["*"]  # internal only; no C name

[[module]]
name = "__div"
file = "math/__div.s"
deps = []
provides = []

[[module]]
name = "__abs"
file = "math/__abs.s"
deps = []
provides = ["abs"]

[[module]]
name = "__min"
file = "math/__min.s"
deps = []
provides = ["min"]

[[module]]
name = "__puts"
file = "io/__puts.s"
deps = ["__putchar"]
provides = ["puts"]

[[module]]
name = "__draw_string"
file = "screen/__draw_string.s"
deps = ["__draw_char"]
provides = ["draw_string", "print_at"]

[[module]]
name = "__printf"
file = "io/hack_printf.c"   # compiled C file
deps = ["__putchar", "__itoa", "__puts"]
provides = ["printf"]
```

### How the Linker Resolves Dependencies

1. Code generator scans call sites in reachable user functions.
2. For each called name, look up `provides` in the manifest to find the
   module. Mark that module as "needed".
3. Follow each needed module's `deps` recursively.
4. Emit only the needed modules, in dependency order (topological sort).

This replaces the current `BuiltinKind` + `used_builtins: HashSet<BuiltinKind>`
mechanism with a data-driven manifest.

---

## Assembly Module Format (`.s` files)

Each `.s` file is valid Hack assembly with a single additional convention:

```asm
// MODULE: __mul
// PROVIDES: *
// DEPS:
//
// Multiply R13 * R14 -> R13. Return via R3.
(__mul)
  @R15
  M=0
  ...
```

The header comment declares the module identity. This allows the linker to
parse dependencies without a separate manifest file (both approaches work;
the manifest is cleaner for compiled C modules).

### Calling Convention for Runtime Functions

All runtime subroutines use the **R3 return convention** (same as today):
- Caller places return address in R3 before jumping
- Callee jumps to `@R3; A=M; 0;JMP` to return
- Arguments: in dedicated registers (R13, R14, R15) for simple 1–3 arg functions
- Results: in R13 (primary) and R15 (secondary, e.g. div remainder)
- Scratch: R5–R12 available inside the subroutine (must be documented per function)

This convention is already established in the codebase. **Do not change it.**

---

## Mixed C/Assembly Strategy

### Which functions should be in assembly

Functions that require direct hardware access or are too low-level to express
in the C subset the compiler itself supports:

| Function | Reason for assembly |
|----------|-------------------|
| `__mul`, `__div` | Performance-critical, R3-convention |
| `__draw_pixel`, `__clear_pixel` | Bit-manipulation in screen word |
| `__draw_char`, `__draw_string` | Complex address arithmetic |
| `__key_pressed` | Reads RAM[24576] directly |
| `__alloc` | Manipulates heap pointer directly |
| `__sys_wait` | Busy-wait counted loop |
| `__fill_screen`, `__clear_screen` | Word-at-a-time screen fill |

### Which functions can be in C

Functions that are algorithms with no special hardware access:

| Function | Notes |
|----------|-------|
| `abs(x)` | `return x < 0 ? -x : x;` |
| `min(a,b)` | `return a < b ? a : b;` |
| `max(a,b)` | `return a > b ? a : b;` |
| `strcpy(d,s)` | Standard byte loop |
| `strcmp(a,b)` | Standard byte loop |
| `strcat(d,s)` | Walk to end of d, then strcpy |
| `strlen(s)` | Already exists as asm; C version equally viable |
| `itoa(n,buf,base)` | Pure algorithm |
| `printf(fmt,...)` | Calls putchar/itoa/puts |
| `draw_rect(x,y,w,h)` | Calls draw_pixel (or fill_rect as line loops) |

### Compiling C runtime files

The compiler is bootstrapped: `hack_math.c`, `hack_string.c`, etc. are
compiled by hack_cc itself. They have access to all builtins that are already
in assembly. The build step is:

```
cargo run --bin hack_cc -- src/runtime/c/hack_math.c --emit-asm-only
```

The resulting assembly is included in the library. Alternatively, the build
system inlines them at link time by compiling all C sources together.

Since these files may not use `main`, we need a `--library` flag that
skips the bootstrap and `main` call, and instead just emits the function
bodies. Already partially supported (the compiler just needs to not require
`main`).

---

## Heap Allocator Design

The simplest allocator is a bump allocator:

```
heap_ptr: RAM[some_address] = HEAP_BASE

void* malloc(int n) {
    int p = *heap_ptr;
    *heap_ptr += n;
    return (void*)p;
}
```

- `HEAP_BASE` = 2048 (above the stack which lives at 256–2047)
- `HEAP_LIMIT` = 15328 (FONT_BASE — the first word of the font table)
- Stack and heap both grow toward the middle:
  Stack grows up from 256; heap grows up from 2048.
  If the stack overflows into the heap, that's UB — same as C.

For Jack game porting, a bump allocator is sufficient because most games
allocate game objects once at startup and never free them.

A proper free-list allocator can be added later as `__alloc_v2.s`.

---

## `<hack.h>` Standard Header

A header file bundling all library declarations:

```c
// hack.h — hack_cc standard library declarations
#ifndef HACK_H
#define HACK_H

// Math
int abs(int x);
int min(int a, int b);
int max(int a, int b);

// String
int strlen(char *s);
char *strcpy(char *dst, char *src);
int strcmp(char *a, char *b);
char *strcat(char *dst, char *src);
void itoa(int n, char *buf, int base);

// I/O
int putchar(int c);
int puts(char *s);
int printf(char *fmt, ...);

// Keyboard
int key_pressed(void);   // returns current key code, 0 if none

// Screen — pixels
void draw_pixel(int x, int y);
void clear_pixel(int x, int y);
void fill_screen(void);
void clear_screen(void);
void draw_line(int x1, int y1, int x2, int y2);
void draw_rect(int x, int y, int w, int h);
void fill_rect(int x, int y, int w, int h);
void draw_circle(int cx, int cy, int r);
void fill_circle(int cx, int cy, int r);

// Screen — text
void draw_char(int col, int row, int c);
void draw_string(int col, int row, char *s);
void print_at(int col, int row, char *s);

// Memory
void *malloc(int n);
void free(void *p);     // no-op in bump allocator

// System
void sys_wait(int ms);  // approximate busy-wait

#endif
```

This file lives at `src/hack.h` (or `include/hack.h`) and is `#include`d
by user programs. Forward declarations allow the checker to type-check calls.

---

## Build System Changes Required

1. **`cargo build` step**: compile `.c` runtime files → assembly snippets.
   Store the results in `src/runtime/compiled/`.
2. **Linker scan**: after codegen, walk the call graph against the manifest
   and determine the needed module set.
3. **Assembly**: concatenate selected runtime `.s` files + user code, assemble.
4. **`--library` mode**: compile without requiring `main`; emit named functions only.

Alternatively (simpler, correct-by-construction): embed the runtime in the
binary as today, but generate the Rust emit-strings from the `.s` source
files at compile time via a build script (`build.rs`). This avoids changing
the binary format while still letting us write runtime code in readable
assembly files.

---

## Emulator Changes for Keyboard Support

The emulator (`hack_emu.rs`) needs to wire keyboard input to RAM[24576]:

- On Windows: use `crossterm` or `winapi` for non-blocking key reads.
- Each emulator cycle, check for a pending keypress; write the nand2tetris
  key code to RAM[24576] (0 if no key, or the code while held).
- nand2tetris key codes match ASCII for printable characters; special keys
  use values 128–140 (arrow keys, backspace, enter, etc.).
- The `--screen` PPM output mode doesn't need keyboard support; add a
  `--interactive` mode that opens a terminal UI.

---

## Phased Rollout

### Phase A: Low-hanging fruit (no build system changes)
Write new builtins directly in `codegen.rs` using the existing pattern:
- `abs`, `min`, `max` (inline codegen, no subroutine needed)
- `strcpy`, `strcmp`, `strcat` (subroutines, add to BuiltinKind)
- `key_pressed` (read RAM[24576], trivial inline)
- `draw_line`, `draw_rect`, `fill_rect` (subroutines)
- `itoa` (subroutine)
- `malloc`/`free` bump allocator (subroutine + global heap_ptr)
- `sys_wait` (subroutine)

### Phase B: Readable assembly sources
Move each runtime subroutine to a `.s` file in `src/runtime/`.
Write a `build.rs` that reads them and generates the Rust emit-strings.
The behavior is identical to Phase A but the source is readable.

### Phase C: C-compiled runtime modules
Implement `hack_string.c`, `hack_printf.c`, `hack_math.c` in C.
Add `--library` flag to the compiler.
Build these as part of `cargo build` and link them in.

### Phase D: Manifest-driven linker
Replace `BuiltinKind` + manual `scan_builtins` with the manifest approach.
Add `runtime.toml` and a proper dependency resolver.
