# hack_cc

A C compiler targeting the [Hack CPU](https://www.nand2tetris.org/) from the nand2tetris course. Translates a subset of C into Hack assembly or machine code, runnable in the bundled emulator or the nand2tetris CPU Emulator.

## Building

```bash
cargo build --release
```

Binaries produced:
- `target/release/hack_cc` — the compiler
- `target/release/hack_ld` — the linker (for separate compilation)
- `target/release/hack_emu` — the emulator

## Compiling and Running

### One-step compile

```bash
hack_cc input.c -o program.asm
hack_emu program.asm
```

### Separate compilation and linking

```bash
hack_cc -c -I include foo.c          # → foo.s
hack_cc -c -I include bar.c          # → bar.s
hack_ld foo.s bar.s -o program.asm   # → program.asm
hack_emu program.asm
```

### Compiler flags

| Flag | Meaning |
|------|---------|
| `-o <file>` | Output file (extension infers format) |
| `-f asm\|hack\|hackem\|tst` | Output format (overrides extension) |
| `-c` | Compile only — produce a `.s` object file |
| `-I <dir>` | Add include search directory (repeatable) |
| `-L <dir>` | Add library search directory (repeatable) |
| `-D NAME[=VALUE]` | Pre-define a preprocessor macro (repeatable) |

### Output formats

| Format | Extension | Description |
|--------|-----------|-------------|
| `asm` | `.asm` | Human-readable Hack assembly with bootstrap data-init code |
| `hack` | `.hack` | nand2tetris binary (one 16-bit instruction per line as `0`/`1` text) |
| `hackem` | `.hackem` | Compact binary with separate `ROM@` / `RAM@` sections; faster to load in the emulator |
| `tst` | `.tst` | nand2tetris test script + companion `.hack` binary |

The `hackem` format is recommended for programs with large data sections (fonts, screen buffers) because it loads RAM directly instead of running thousands of bootstrap instructions.

### Emulator options

```bash
hack_emu program.asm                        # run to halt
hack_emu program.asm --max-cycles 5000000   # limit execution
hack_emu program.asm --dump-ram 64          # print first 64 RAM words on exit
hack_emu program.asm --screen screen.ppm    # save screen as PPM image
hack_emu program.asm --trace                # print every instruction
hack_emu program.asm --quiet                # suppress normal output
```

The emulator exits with the value of RAM[0] (the final stack pointer) as its exit code — useful for testing return values.

### Screen output mode

By default `putchar`/`puts` write to the emulator output port (RAM[32767]). To route them through the on-screen text console instead (needed for the nand2tetris CPU Emulator):

```bash
hack_cc -D HACK_OUTPUT_SCREEN -I include program.c -o program.asm
```

---

## Supported C

### Types

| Type | Width | Notes |
|------|-------|-------|
| `int` | 16 bits | Signed, the native Hack word size |
| `char` | 16 bits (stored), 8-bit arithmetic | Sign-extends and truncates to [-128, 127] on assignment |
| `long` | 32 bits (2 words) | Signed 32-bit; supports all arithmetic and comparison |
| `int *`, `char *`, etc. | 16 bits | Pointer; arithmetic and dereference supported |
| `int arr[N]` | N × 16 bits | Stack or global; decays to pointer in expressions |
| `struct` | sum of fields | Passed and returned by value |
| `void` | — | Return type only |

Type qualifiers `const`, `signed`, `unsigned`, `extern`, `static`, `short` are accepted syntactically. `unsigned` arithmetic is not distinguished from signed — all values are treated as signed 16-bit or 32-bit.

### Operators

```
Arithmetic:    +  -  *  /  %
Bitwise:       &  |  ^  ~  <<  >>
Comparison:    ==  !=  <  <=  >  >=
Logical:       &&  ||  !
Assignment:    =  +=  -=  *=  /=  %=  &=  |=  ^=  <<=  >>=
Increment:     ++  --  (prefix and postfix)
Pointer:       *  &  ->  []
Cast:          (type) expr
Sizeof:        sizeof(type)  sizeof expr
```

### Control flow

```c
if (cond) { ... } else { ... }
while (cond) { ... }
do { ... } while (cond);
for (init; cond; step) { ... }
switch (expr) { case N: ... default: ... }
break;  continue;  return expr;
```

### Functions

- Recursive calls supported.
- Variadic functions (`...`) accepted syntactically (no `va_list` support).
- Forward declarations supported.
- Multiple source files accepted: `hack_cc file1.c file2.c` merges them before compilation.

### Preprocessor

```c
#define NAME value
#define NAME(a, b) body        // function-like macro
#undef NAME
#ifdef / #ifndef / #else / #endif
#if expr / #elif expr          // integer constant expressions + defined()
#include "path"                // relative include
#include <path>                // system include (search -I directories)
```

### Storage classes

```c
static int x;          // file-scope: only visible in this translation unit
static int counter;    // function-scope: retains value across calls (like global)
extern int g;          // declares externally defined global
```

### Known limitations

- No floating-point.
- No 64-bit literals (constants must fit in 16 bits; `long` constants beyond 32767 are not supported in source).
- `unsigned` arithmetic behaves as signed.
- No `goto`.
- Nested struct initialisers (`{ { ... } }`) not supported.
- `signed TYPE` as a standalone type specifier (e.g. `signed int`) not supported in all positions.

---

## Runtime library

Include `<hack.h>` for declarations. See [RUNTIME.md](RUNTIME.md) for the full API.

```c
#include <hack.h>

int main(void) {
    puts("hello");
    draw_string(5, 3, "world");
    return 0;
}
```

Only the library functions your program actually calls are linked in — unused code is not emitted.

---

## Memory layout

```
RAM[0]         SP   — stack pointer (initialised to 256)
RAM[1]         LCL  — local variable base for current frame
RAM[2]         ARG  — argument base for current frame
RAM[3]         THIS
RAM[4]         THAT
RAM[5-12]      Temp registers
RAM[13-15]     R13/R14/R15 — scratch registers
RAM[16+]       Global variables and string literals
RAM[256+]      Call stack
RAM[15328]     8×11 font bitmap (96 chars × 11 rows = 1056 words)
RAM[16384]     Screen memory (512×256 pixels, 32 words per row)
RAM[24576]     Keyboard register
RAM[32767]     Output port (putchar writes here; read by hack_emu)
```

---

## Tests

```bash
cargo test
```

The test suite runs ~165 tests: assembler unit tests, emulator unit tests, end-to-end compile+run tests, and a sample of the [nand2tetris C compiler test suite](https://github.com/nlsandler/writing-a-c-compiler-tests) (chapters 1–10).

