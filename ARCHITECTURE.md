# hack_cc Architecture

This document describes the internal structure of `hack_cc`.

## Pipeline overview

```
C source text
    │
    ▼
┌─────────────┐
│ Preprocessor│  #define, #include, #ifdef, function-like macros
└─────────────┘
    │
    ▼
┌───────────┐
│   Lexer   │  Produces a token stream with line/column info
└───────────┘
    │
    ▼
┌───────────┐
│  Parser   │  Recursive-descent; builds a typed AST
└───────────┘
    │
    ▼
┌───────────┐
│   Sema    │  Type-checks, resolves symbols, computes offsets
└───────────┘
    │
    ▼
┌───────────┐
│  Codegen  │  Walks annotated AST; emits Hack assembly text
└───────────┘
    │
    ▼
┌───────────┐
│  Linker   │  Symbol-scan: pulls in runtime .s files on demand
└───────────┘
    │
    ▼
┌───────────┐
│ Assembler │  Two-pass; resolves labels; produces 16-bit words
└───────────┘
    │
    ▼
┌───────────┐
│  Output   │  Formats: asm / hack / hackem / tst
└───────────┘
```

Source files: `src/preprocessor.rs`, `src/lexer.rs`, `src/parser.rs`, `src/sema.rs`, `src/codegen.rs`, `src/linker.rs`, `src/assembler.rs`, `src/output.rs`, `src/lib.rs`.

---

## Preprocessor (`preprocessor.rs`)

Text-level macro expansion runs before the lexer. It supports:

- Object-like and function-like `#define`
- `#undef`, `#ifdef`, `#ifndef`, `#if`, `#elif`, `#else`, `#endif`
- `#include "path"` (relative) and `#include <path>` (searched via `-I` directories)

`-D NAME=VALUE` flags are injected as pre-defined macros before the file is processed.

---

## Lexer (`lexer.rs`)

Converts preprocessed text into a flat `Vec<Token>`, each carrying its kind, source text, line, and column. The lexer handles:

- Integer literals (decimal, hex `0x…`, octal `0…`, character `'x'`)
- String literals with escape sequences
- All C keywords, operators, and punctuation
- Line comments `//` and block comments `/* */`

Integer literals are checked against the 15-bit Hack limit (max 32767) during lexing; values that exceed it are rejected (platform constraint).

---

## Parser (`parser.rs`)

Recursive-descent parser produces an untyped AST. Key AST types:

| Type | Represents |
|------|-----------|
| `Program` | List of top-level declarations (functions + globals + struct defs) |
| `FuncDef` | Function: name, return type, parameter list, body (`Vec<Stmt>`) |
| `Stmt` | `If`, `While`, `DoWhile`, `For`, `Return`, `Break`, `Continue`, `Switch`, `Block`, `Expr`, `VarDecl`, `StructDecl` |
| `Expr` | `Literal`, `Ident`, `BinOp`, `UnOp`, `Call`, `Cast`, `Subscript`, `Field`, `Arrow`, `Sizeof`, `InitList`, … |
| `Type` | `Void`, `Int`, `Char`, `Long`, `Ptr(T)`, `Array(T, N)`, `Struct(name)` |

The parser handles:
- Compound assignment operators, pre/post increment/decrement
- `->` and `.` field access
- `sizeof`, cast expressions, comma operator in `for` init
- Array declarators and initialiser lists `{ 1, 2, 3 }`
- `static`, `extern`, `typedef`, `enum` (enums lower to integer constants)

---

## Semantic analysis (`sema.rs`)

`analyse(program)` returns a `SemaResult`, which annotates each function with:

- A flat `vars: HashMap<String, VarInfo>` mapping every local/param name to a `VarStorage` (local slot index, param slot index, or global symbol name).
- `n_locals`: number of stack words needed for local variables.
- Resolved types on every declaration.

Also produces:
- `globals`: list of `(symbol, type, Option<init>)` for global variables.
- `string_literals`: deduplicated list of string constants with their assembler symbol names.
- `struct_defs`: field lists for all `struct` definitions, used to compute offsets and sizes throughout codegen.

`type_size(ty, struct_defs)` returns the size in Hack words: `int`/`char`/pointer = 1, `long` = 2, `struct` = sum of field sizes, `array` = N × element size.

---

## Code generation (`codegen.rs`)

`Codegen` walks the `SemaResult` and emits Hack assembly as a `String`.

### Calling convention

The nand2tetris Jack VM calling convention is used:

```
Caller:
  push arg_0          ← first arg pushed first
  push arg_1
  ...
  push arg_n-1
  call FuncName n_args

Callee frame layout (top of stack at entry):
  [return address]    ← pushed by call trampoline
  [saved LCL]
  [saved ARG]
  [saved THIS]
  [saved THAT]
  [local 0]           ← LCL points here
  [local 1]
  ...

Return convention:
  Value in top-of-stack → reused as return value by __vm_return trampoline.
```

`long` (32-bit) values occupy two consecutive stack words: high word on top, low word below.

For the call sequence itself, `hack_cc` emits inline assembly that:
1. Pushes each argument.
2. Emits `@__vm_call`, `D=A`, and the encoded `(function, n_args)` payload.
3. Calls through the `__vm_call` trampoline in `lib/sys/__vm_call.s`.

Returns are similarly handled via `__vm_return` / `__vm_return_long`.

### Register usage

| Register | Role |
|----------|------|
| `R13` | General scratch; temporarily holds expression values during assignment |
| `R14` | Call trampoline scratch; also holds `long` high word during operations |
| `R15` | Scratch for multi-step operations (e.g. signed char truncation, `long` stores) |

### Runtime arithmetic helpers

Multiplication and division are not native Hack instructions; they are implemented as subroutines using the `R3`-convention (caller saves return address in R3):

- `__mul (R13, R14 → R13)` — 16-bit signed multiply
- `__div (R13, R14 → R13, R14)` — 16-bit signed divide; R13 = quotient, R14 = remainder
- `__lmul`, `__ldiv`, `__ladd`, `__lsub`, `__lneg`, `__lshl`, `__lshr` — 32-bit (`long`) variants

### Writing runtime `.s` files

Runtime functions written in Hack assembly follow one of two conventions:

**VM convention** — used by the public wrapper functions in `lib/` (e.g. `strlen.s`, `malloc.s`). Arguments arrive via the VM stack (ARG[0], ARG[1], …) and the return value is left on the stack for `__vm_return` to handle. These are called exactly like C functions.

**R3 convention** — used by internal helpers (names beginning with `__`, e.g. `__mul`, `__div`, `__ladd`). The caller stores the return address in R3 before jumping; the helper jumps to `R3` on return. Arguments are passed in R13/R14; the result is left in R13 (and R14 for 32-bit results). This avoids the overhead of a full VM frame for short arithmetic helpers.



Codegen walks the call graph starting from `main` (BFS). Functions not reachable from `main` are not emitted. This is done in `collect_calls_stmt/expr` before emitting code.

### Global data and string literals

The `SemaResult` includes all globals and string literals. In `asm`/`hack`/`tst` formats these are emitted as bootstrap code (data-init instructions before `main`). In `hackem` format they become `RAM@` sections loaded directly by the emulator.

The 8×11 font table (1056 words, 96 printable ASCII characters) is likewise emitted only if `draw_char` or `draw_string` is used.

---

## Linker (`linker.rs`)

After codegen, the assembly text still contains `@symbol` references to runtime functions that have not been defined. The linker resolves these by:

1. Building an index of the `lib/` directory tree, keyed on `// PROVIDES:` (or `.provides`) tokens on the first line of each `.s` file.
2. Scanning the generated assembly for undefined `@symbol` references (symbols that appear as `@sym` but have no matching `(sym)` label).
3. For each undefined symbol, appending the corresponding `.s` file to the assembly text.
4. Repeating until no more unresolved references remain (transitive pull-in).

This means every runtime module is self-contained and no registration in Rust is needed — adding a new library function is just adding a `.s` file with the right `// PROVIDES:` header.

Library search order: `HACK_LIB` env var → `./lib/` (relative to cwd) → `lib/` next to the executable. Override with `-L <dir>`.

---

## Assembler (`assembler.rs`)

A standard two-pass Hack assembler:

- **Pass 1**: scan for label definitions `(LABEL)` and build a symbol table with their ROM addresses.
- **Pass 2**: translate each instruction.
  - A-instruction `@symbol` or `@N`: emits `0vvvvvvvvvvvvvvv` (15-bit value).
  - C-instruction `dest=comp;jump`: encodes the 3-bit dest, 7-bit comp, and 3-bit jump fields.

Named variables (first `@symbol` that is not a label) are assigned addresses starting at `next_var_addr` (passed in from codegen to avoid collision with global variables, which also start at RAM[16]).

---

## Output (`output.rs`)

`emit(program, format)` converts a `CompiledProgram` (assembled ROM words + RAM data) into the requested format:

- **`asm`**: Prepends bootstrap instructions that initialise SP and write RAM data, then appends the assembly text.
- **`hack`**: Same as `asm` but assembled to binary text.
- **`hackem`**: Writes `ROM@` sections for code and `RAM@` sections for data. Data is written directly to RAM addresses; no bootstrap instructions needed.
- **`tst`**: Generates a nand2tetris test script with `set RAM[n] v,` preamble, and writes a companion `.hack` binary.

---

## Emulator (`src/bin/hack_emu.rs`)

A cycle-accurate interpreter for the Hack CPU:

- 32 K ROM, 32 K RAM (16-bit words each).
- Supports both `.asm` (assembled on the fly) and `.hackem` input.
- Output port at RAM[32767]: each write is captured and printed as a character.
- Screen memory at RAM[16384–24575]: can be saved to a PPM image with `--screen`.
- Keyboard at RAM[24576]: not simulated (reads 0).
- Halts when PC reaches the halt address (an infinite loop `(__end)`).
- Exit code = value of RAM[0] at halt (the stack pointer — useful for unit tests).

---

## Separate compilation (`hack_ld`)

`hack_cc -c` compiles a single `.c` file to a `.s` object file without linking or bootstrap. The object file is valid Hack assembly plus metadata directives:

```
// PROVIDES: <symbols defined in this file>
// DATA: <global variable initialisers>
// NEXT_VAR: <next free RAM address>
```

`hack_ld` reads one or more `.s` files, merges their data, generates bootstrap, runs the runtime linker, and emits the final output — same formats as `hack_cc`.

---

## The `.s` file format

`.s` files are used for two distinct purposes: runtime library modules (in `lib/`) and object files produced by `hack_cc -c`. Both are plain Hack assembly text augmented with leading directive lines. Directive lines are valid assembler no-ops (they begin with `.` or `//` so the assembler ignores them); they carry metadata consumed by the linker and `hack_ld`.

### Directive reference

| Directive | Who uses it | Meaning |
|-----------|-------------|---------|
| `.provides sym1 sym2 …` | Library files and object files | Declares the assembler labels this file defines. The linker uses this to build its index; a file is pulled in only when one of its provided symbols is referenced. |
| `.data sym value` | Object files | Declares one RAM word: symbol name and its initial integer value. `hack_ld` collects all `.data` entries in file order, assigns consecutive RAM addresses starting at 16, and emits bootstrap code to initialise them. |
| `// DEPS: sym1 sym2 …` | Library files (informational) | Documents which R3-convention helpers this file calls. Not parsed; for human reference only. |

### Library file structure

```
.provides strlen
// DEPS: __strlen
(strlen)
    // ... Hack assembly implementing the VM-convention wrapper ...
```

Every library file must have `.provides` as its **first line**. The linker scans only that line when building the index; the rest of the file is loaded verbatim when the symbol is needed.

### Object file structure

An object file produced by `hack_cc -c` looks like:

```
.provides main greet             ← symbols defined in this translation unit
.data __g_count 0                ← scalar global (name, initial value)
.data __str_0 72                 ← string literal "Hello" — first char
.data __str_0_1 101
.data __str_0_2 108
.data __str_0_3 108
.data __str_0_4 111
.data __str_0_5 0                ← null terminator
(main)
    // ... generated assembly ...
(greet)
    // ... generated assembly ...
```

Multi-word values (e.g. arrays, structs, `long` variables) emit one `.data` line per word, with consecutive suffixed names (`sym`, `sym_1`, `sym_2`, …).

`hack_ld` processes all `.data` entries in file order, assigning each a RAM address (starting at 16), then emits bootstrap instructions that write the initial values before calling `main`.

### Symbol naming conventions

| Prefix | Example | Origin |
|--------|---------|--------|
| `__g_` | `__g_count` | C global variable |
| `__sl_f_` | `__sl_f_x` | `static` local variable in function `f` |
| `__str_N` | `__str_0` | String literal N (with `_K` suffix for word K) |
| `__` | `__mul`, `__strlen` | Internal R3-convention helper |
| *(bare)* | `strlen`, `malloc` | VM-convention public function |
