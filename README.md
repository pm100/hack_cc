# hack_cc

A C compiler targeting the [Hack CPU](https://www.nand2tetris.org/) from the nand2tetris course. Translates a subset of C into Hack assembly or machine code, which can be executed on the included emulator or loaded directly into a nand2tetris simulator.

## Features

- **Full compilation pipeline**: Lexing → Parsing → Semantic Analysis → Code Generation → Assembly
- **Type system**: `int`, `char`, `void`, pointers, arrays, and structs
- **Control flow**: `if`/`else`, `while`, `for`, `return`
- **Functions**: recursive calls, multiple parameters, local variables
- **Global variables** with initializers and string literals
- **Library system**: only the runtime helpers your program actually uses are emitted — unused functions and the 768-word font table are excluded automatically
- **Multiple output formats**: Hack assembly, nand2tetris binary, hackem binary, nand2tetris test scripts
- **Built-in graphics**: draw characters and strings using an embedded 8×8 bitmap font
- **Runtime helpers**: 16-bit signed multiplication, division, and modulo

## Architecture

```
C Source → Lexer → Parser → Sema → Codegen → Assembler → Output
```

| Module | Role |
|--------|------|
| `lexer.rs` | Tokenises source text |
| `parser.rs` | Builds an Abstract Syntax Tree (AST) |
| `sema.rs` | Type-checks and resolves symbols |
| `codegen.rs` | Emits Hack assembly; dead-code eliminates unused functions and runtime helpers |
| `assembler.rs` | Two-pass Hack assembler: assembly text → 16-bit machine words |
| `output.rs` | Converts a compiled program to the requested output format |
| `bin/hack_emu.rs` | Assembles and executes Hack assembly; renders screen to PPM |

## Building

```bash
cargo build --release
```

## Usage

### Compile a C file

```bash
# Default: produce Hack assembly (.asm)
./target/release/hack_cc input.c

# Explicit output path
./target/release/hack_cc input.c -o program.asm

# Choose output format
./target/release/hack_cc input.c -f asm     # Hack assembly (default)
./target/release/hack_cc input.c -f hack    # nand2tetris .hack binary
./target/release/hack_cc input.c -f hackem  # hackem emulator format
./target/release/hack_cc input.c -f tst     # nand2tetris test script
```

The output file extension is inferred from the format when `-o` is omitted:

| Format | Extension |
|--------|-----------|
| `asm` | `.asm` |
| `hack` | `.hack` |
| `hackem` | `.hackem` |
| `tst` | `.tst` (+ companion `.hack`) |

### Run with the emulator

```bash
./target/release/hack_emu program.asm
./target/release/hack_emu program.asm --max-cycles 5000000
./target/release/hack_emu program.asm --dump-ram 64
./target/release/hack_emu program.asm --screen screen.ppm
./target/release/hack_emu program.asm --trace
```

## Output Formats

### `asm` — Hack Assembly (default)

Human-readable Hack assembly text. Global variable initializers, string literals, and the font table are inlined as bootstrap code that runs before `main`. Compatible with the nand2tetris CPU Emulator and `hack_emu`.

```asm
// Bootstrap
@256
D=A
@SP
M=D
...
(main)
@10
D=A
...
```

### `hack` — nand2tetris Binary

One 16-bit instruction per line, encoded as ASCII `0`/`1` characters. This is the standard `.hack` file format accepted by the nand2tetris CPU Emulator. Data initialisation runs as part of the bootstrap code (same as `asm` format, just assembled to binary).

```
0000000100000000
1110110000010000
1110001100001000
...
```

### `hackem` — hackem Emulator Format

A compact binary format for the [hackem](../hackem) emulator. Code and data are separated into `ROM@` and `RAM@` sections, so global variables, string literals, and the font table are loaded directly into RAM rather than executed as code. This eliminates thousands of bootstrap instructions compared to the `asm`/`hack` formats.

**File structure:**

```
hackem v1.0 0x<halt_address>

ROM@<hex_addr>
<hex_word>
<hex_word>
...

RAM@<hex_addr>
<hex_word>
<hex_word>
...
```

- **Header**: `hackem v1.0 0x<halt>` — `<halt>` is the ROM address of the `(__end)` infinite loop; the emulator stops execution there.
- **`ROM@<addr>`**: Loads subsequent hex words into ROM starting at `<addr>`. Multiple `ROM@` sections are allowed.
- **`RAM@<addr>`**: Loads subsequent hex words into RAM starting at `<addr>`. Contiguous data is grouped into a single section; gaps larger than 16 words start a new section.
- Words are 4-digit lowercase hex (e.g. `ec10`).

**Example:**
```
hackem v1.0 0x0042

ROM@0000
0100
ec10
0000
e308
0042
ec10
0000
fc20

RAM@0010
0007
0000
0004
```

### `tst` — nand2tetris Test Script

Produces two files:

1. **`<name>.tst`** — A test script for the nand2tetris CPU Emulator. Global data is pre-loaded via `set RAM[n] v,` commands; the script then runs the program and captures output.
2. **`<name>.hack`** — The compiled binary loaded by the `.tst` script.

**Example `.tst` output:**
```
// Auto-generated nand2tetris test script
load prog.hack,
output-file prog.out,
output-list RAM[0]%D1.6.1;
set RAM[16] 7,
set RAM[17] 4,
set PC 0,
repeat 100000 {
  ticktock;
}
output;
```

## Library System (Dead-Code Elimination)

The compiler automatically omits any runtime helpers your program doesn't use. This keeps the output small and fast to simulate.

| Used feature | Helper emitted |
|---|---|
| `*` operator | `__mul` |
| `/` or `%` operator | `__div` |
| `puts()` | `__puts` |
| `strlen()` | `__strlen` |
| `draw_pixel()` | `__draw_pixel` |
| `clear_pixel()` | `__clear_pixel` |
| `fill_screen()` | `__fill_screen` |
| `clear_screen()` | `__clear_screen` |
| `draw_char()` | `__draw_char` + 768-word font table |
| `draw_string()` / `print_at()` | `__draw_string` + `__draw_char` + font table |

User-defined functions are also eliminated if they are not reachable from `main` (dead function elimination via call-graph BFS).

## Supported C Subset

```c
// Types
int x;
char c;
int *p;
struct Point { int x; int y; };

// Operators
+ - * / % & | ~ ! == != < > <= >= && || ++ -- += -= -> []

// Control flow
if (cond) { ... } else { ... }
while (cond) { ... }
for (init; cond; step) { ... }
return expr;

// Functions
int add(int a, int b) { return a + b; }
```

**Not supported:** hex literals (`0xFF`), `^` (XOR), `?:` (ternary), forward declarations, `switch`, `do`/`while`.

## Built-in Functions

| Function | Description |
|----------|-------------|
| `putchar(c)` | Write a character to the output port |
| `puts(s)` | Print a null-terminated string followed by a newline |
| `strlen(s)` | Return the length of a string |
| `draw_char(col, row, code)` | Draw an ASCII character at 8×8 grid position |
| `draw_string(col, row, str)` | Draw a null-terminated string at grid position |
| `print_at(col, row, str)` | Alias for `draw_string` |
| `draw_pixel(x, y)` | Set a pixel black (screen coordinates) |
| `clear_pixel(x, y)` | Set a pixel white |
| `fill_screen()` | Fill the entire screen black |
| `clear_screen()` | Clear the entire screen white |

## Memory Layout

```
RAM[0]       SP  (stack pointer, starts at 256)
RAM[1]       LCL (local frame base)
RAM[2]       ARG (argument base)
RAM[3]       THIS
RAM[4]       THAT
RAM[5-12]    Temp registers
RAM[13-15]   Scratch (R13/R14/R15)
RAM[16+]     Global variables and string literals
RAM[256+]    Call stack
RAM[15616]   Embedded 8×8 font bitmap (FONT_BASE, 768 words, 96 printable chars × 8 rows)
RAM[16384]   Screen memory (512×256 pixels, 32 words per row)
RAM[24576]   Keyboard input
```

## Examples

```bash
# Compile and run a program
cargo run --bin hack_cc -- test.c
cargo run --bin hack_emu -- test.asm --dump-ram 20

# Produce a nand2tetris binary and test script
cargo run --bin hack_cc -- test.c -f hack
cargo run --bin hack_cc -- test.c -f tst

# Produce hackem format (code + data separated)
cargo run --bin hack_cc -- test_font.c -f hackem

# Render text on screen
cargo run --bin hack_cc -- test_font.c
cargo run --bin hack_emu -- test_font.asm --screen screen.ppm
```

## Tests

```bash
cargo test
```

91 tests: 21 assembler unit tests and 70 end-to-end integration tests that compile C snippets and execute them in the built-in emulator.
