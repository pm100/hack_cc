# hack_cc

A C compiler targeting the [Hack CPU](https://www.nand2tetris.org/) from the nand2tetris course. Translates a subset of C into Hack assembly language, which can be executed on the included emulator.

## Features

- **Full compilation pipeline**: Lexing → Parsing → Semantic Analysis → Code Generation
- **Type system**: `int`, `char`, `void`, pointers, arrays, and structs
- **Control flow**: `if`/`else`, `while`, `for`, `return`
- **Functions**: recursive calls, multiple parameters, local variables
- **Global variables** and string literals
- **Built-in graphics**: draw characters and strings using an embedded 8×8 bitmap font
- **Runtime helpers**: 16-bit signed multiplication and division

## Architecture

```
C Source → Lexer → Parser → Sema → Codegen → Hack Assembly
```

| Module | Role |
|--------|------|
| `lexer.rs` | Tokenises source text |
| `parser.rs` | Builds an Abstract Syntax Tree (AST) |
| `sema.rs` | Type-checks and resolves symbols |
| `codegen.rs` | Emits Hack assembly |
| `bin/hack_emu.rs` | Assembles and executes Hack assembly |

## Building

```bash
cargo build --release
```

## Usage

### Compile a C file

```bash
./target/release/hack_cc input.c
# output defaults to out.asm

./target/release/hack_cc input.c --output program.asm
```

### Run with the emulator

```bash
./target/release/hack_emu out.asm
./target/release/hack_emu out.asm --max-cycles 5000000
./target/release/hack_emu out.asm --dump-ram 64
./target/release/hack_emu out.asm --screen screen.ppm
./target/release/hack_emu out.asm --trace
```

## Supported C Subset

```c
// Types
int x;
char c;
int *p;
int arr[10];
struct Point { int x; int y; };

// Operators
+ - * / % & | ! == != < > <= >= && || ++ -- -> []

// Control flow
if (cond) { ... } else { ... }
while (cond) { ... }
for (init; cond; step) { ... }
return expr;

// Functions
int add(int a, int b) { return a + b; }
```

## Built-in Functions

| Function | Description |
|----------|-------------|
| `putchar(c)` | Write a character to the output port |
| `puts(s)` | Print a null-terminated string + newline |
| `strlen(s)` | Return length of a string |
| `draw_char(col, row, code)` | Draw ASCII character at grid position |
| `draw_string(col, row, str)` | Draw a string at grid position |
| `print_at(col, row, str)` | Alias for `draw_string` |
| `draw_pixel(x, y)` | Set a pixel black |
| `clear_pixel(x, y)` | Set a pixel white |
| `fill_screen()` | Fill the screen black |
| `clear_screen()` | Clear the screen white |

## Memory Layout

```
RAM[0-4]     SP, LCL, ARG, THIS, THAT
RAM[5-12]    Temp registers
RAM[13-15]   Scratch (R13/R14/R15)
RAM[16+]     Global variables and string literals
RAM[256+]    Stack
RAM[15616]   Embedded 8×8 font bitmap (FONT_BASE)
RAM[16384]   Screen memory
RAM[24576]   Keyboard
```

## Examples

```bash
# Compile and run factorial/fibonacci
cargo run --bin hack_cc -- test2.c
cargo run --bin hack_emu -- out.asm --dump-ram 20

# Render text on screen
cargo run --bin hack_cc -- test_font.c
cargo run --bin hack_emu -- out.asm --screen screen.ppm
```
