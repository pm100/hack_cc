# Plan 1: Missing Language Features

## Overview

hack_cc currently compiles a useful subset of C but is missing features
that prevent porting real programs. This plan documents every gap, grouped
by effort and dependency, and proposes a concrete implementation path for
each one.

---

## Already Working

For reference, these already work:
`int`, `char`, `void`, pointers, arrays, structs (value semantics),
`if/else`, `while`, `for`, recursion, `return`,
arithmetic `+ - * / %`, bitwise `& | ^ ~`,
logical `&& || !`, comparisons, ternary `?:`,
`++`/`--` (prefix and postfix, but both act as pre-increment),
`=`, `+=`, `-=`,
`sizeof(type)`, hex literals, string literals, `#define`, `#ifdef`, `#include`.

---

## Tier 1 — Easy, High-Impact

### 1.1 Shift operators `<<` and `>>`

**Where**: lexer + parser + codegen  
**Effort**: ~30 min

- Lexer: add `TokenKind::LtLt` (from `<<`) and `GtGt` (from `>>`).  
  Must be emitted **before** the single `<`/`>` tokens. The existing `<`/`>`
  handling must check for a second identical char first.
- Parser: add `BinOp::Shl` and `BinOp::Shr`.  
  Insert `parse_shift_expr()` between `parse_add_expr` and `parse_rel_expr`
  in the precedence chain.
- Codegen: emit inline Hack assembly. Both are loop-based (no native shift):
  - `x << n`: loop n times, `M = M + M` (doubling).
  - `x >> n`: loop n times, `M = M / 2` — but Hack has no right-shift.
    Use the divide subroutine or a dedicated `__shr` loop.
  - For constant shift amounts (the common case), unroll the loop.
  - For variable amounts, emit a counted loop using R13/R14.
- `BinOp::ShlAssign` (`<<=`) and `ShrAssign` (`>>=`) for completeness.

### 1.2 `do { } while (cond)` loop

**Where**: lexer (reuses `KwDo`), parser, sema, codegen  
**Effort**: ~20 min

- Lexer: add `TokenKind::KwDo`.
- Parser: `Stmt::DoWhile(Box<Stmt>, Expr)`. In `parse_stmt`, handle `do`:
  ```
  advance(); // consume 'do'
  let body = parse_stmt()?;
  expect(KwWhile)?;
  expect(LParen)?;
  let cond = parse_expr()?;
  expect(RParen)?;
  expect(Semicolon)?;
  Ok(Stmt::DoWhile(body, cond))
  ```
- Sema: add `DoWhile` arm to `collect_locals_stmt`,
  `collect_strings_stmt`, `check_calls_stmt`, `scan_builtins_stmt`,
  `alpha_rename_stmt`. All trivial.
- Codegen: emit label before body, body, then evaluate cond, jump back.

### 1.3 `break` and `continue`

**Where**: parser, sema, codegen  
**Effort**: ~45 min (most effort is threading the loop-end label through gen_stmt)

- Lexer: add `TokenKind::KwBreak`, `TokenKind::KwContinue`.
- Parser: `Stmt::Break`, `Stmt::Continue`.
- Sema: add trivial arms everywhere.
- Codegen: `gen_stmt` needs to know the current loop's `l_end` (for break)
  and `l_continue` (for continue, which is the increment label in a `for`
  loop or the condition label in `while`/`do-while`).
  Pass an `Option<(&str, &str)>` — `(break_label, continue_label)` — as
  an extra argument to `gen_stmt`, threaded through all recursive calls.
  `break` emits `@break_label; 0;JMP`. `continue` emits `@continue_label; 0;JMP`.
  Error if used outside a loop.

### 1.4 `switch / case / default`

**Where**: lexer, parser, sema, codegen  
**Effort**: ~1.5 hours

- Lexer: `KwSwitch`, `KwCase`, `KwDefault`.
- Parser: `Stmt::Switch(Expr, Vec<(Option<i32>, Vec<Stmt>)>)`.
  Each arm is `(Some(n), stmts)` for `case n:` or `(None, stmts)` for `default:`.
  Only integer constant case labels (no expression cases for now).
- Codegen: generate a chain of comparisons.  
  Load switch value into R13 once.  
  For each case: `D = R13 - n; @case_N; D;JEQ`.  
  Jump to default (or end) if no match.  
  Emit each case block with fall-through (no implicit break — user must
  write explicit `break`).  
  `break` inside a switch jumps to the switch-end label.

---

## Tier 2 — Moderate Effort

### 2.1 Cast expressions `(type)expr`

**Where**: parser  
**Effort**: ~30 min

The tricky part is disambiguation: `(int)` looks like a parenthesised type
in primary position. Two approaches:
- **Lookahead**: in `parse_primary`, if `(` is followed by a type keyword
  (`int`, `char`, `void`, `struct`), consume it as a cast.
  `Expr::Cast(Type, Box<Expr>)`.
- At runtime on Hack: casts between `int` and `char` are truncation to 8 bits
  (`D = D & 255`). Pointer casts are no-ops.
- `Expr::Cast` arm needed in all recursive walkers (sema, codegen).

### 2.2 Compound assignment `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=`

**Where**: lexer, parser, codegen  
**Effort**: ~30 min

- Lexer: 8 new token kinds (`StarAssign`, `SlashAssign`, etc.).
- Parser: add to `parse_assign_expr`, desugar to `lhs = lhs op rhs`.
- Codegen: already handles AddAssign and SubAssign by desugaring; extend
  the same pattern.

### 2.3 `typedef`

**Where**: lexer, parser  
**Effort**: ~1 hour

- Lexer: `KwTypedef`.
- Parser: maintain a `typedef_map: HashMap<String, Type>`.
  When parsing a type and the current ident is in `typedef_map`, substitute.
  Handle `typedef int foo;` by adding `foo -> Type::Int` to the map.
  This also enables `typedef struct Foo Foo;` patterns.

### 2.4 `enum`

**Where**: lexer, parser  
**Effort**: ~45 min

- Lexer: `KwEnum`.
- Parser: `enum Name { A=0, B, C }` populates a `const_map: HashMap<String, i32>`.
  References to enum values in expressions resolve to their integer value.
  Treat enum variables as `int`.

### 2.5 `unsigned` type modifier

**Where**: lexer, parser  
**Effort**: ~30 min surface, but requires codegen changes for correct semantics

For Hack purposes, `unsigned int` can be treated as `int` (both 16-bit).
The main behavioural difference is in comparisons (unsigned `>=` vs signed `>=`).
- Short path: parse `unsigned` as a modifier but treat the type as `int`.
  Mark it as `Type::UInt` for future correctness; for now emit the same code.

### 2.6 Variadic functions (printf-style)

**Where**: parser, sema, codegen  
**Effort**: ~2 hours

- Parser: parse `...` as a final parameter.
- Sema: mark `FuncDef` as variadic; skip param-count check for calls.
- Codegen: caller pushes extra args normally; callee uses ARG offsets to
  access them. This matches the current calling convention naturally.
  The only hard part is implementing `printf` itself using variadic access.

### 2.7 Multi-dimensional arrays

**Where**: parser, sema, codegen  
**Effort**: ~1 hour

- Parser: parse multiple `[N]` suffixes: `int arr[3][4]`.
  Represent as nested `Type::Array(Type::Array(Type::Int, 4), 3)`.
- Codegen: `arr[i][j]` desugars as `*(arr + i*4 + j)`.
  `gen_addr` for `Index(Index(base, i), j)` needs to know the element stride
  from the type. Already available via `type_size`.

### 2.8 Function pointers (basic)

**Where**: parser, sema, codegen  
**Effort**: ~3 hours

- Parser: `void (*fp)(int)` syntax is complex. Simplest approach: only
  support `typedef void (*Callback)(int)` style, not inline pointer types.
- Codegen: a function pointer is the ROM address of the function label.
  On Hack, ROM addresses are not directly available at runtime (no PIC
  instruction). This would require a jump-table approach, making function
  pointers a label index rather than an address.
  **Recommendation**: defer until truly needed for a specific game port.

---

## Memory Model Change: int = 2 words, POSIX Compliance

### Background

The current compiler maps all scalar types to 1 Hack word (16 bits):
`sizeof(char) == sizeof(int) == sizeof(ptr) == 1`.

This is non-standard. POSIX and the C standard require that the individual
bytes of an `int` be independently addressable as `char`. This means a
`char*` cast of an `int*` must give valid byte access — which is impossible
if int and char are the same width.

For the Unix V6 porting goal this matters concretely: the kernel uses
`char*`/`int*` aliasing in the `u` structure, device drivers, and
filesystem code.

### Proposed Model B: int = 2 words

| Type | Words | Notes |
|------|-------|-------|
| `char` | 1 | stores 0–255 in the low byte of a 16-bit word |
| `int` | 2 | lo word = low byte (0–255), hi word = high byte (0–255) |
| `long` | 4 | two ints, little-endian |
| `ptr` | 1 | a Hack RAM address still fits in one 16-bit word |

`sizeof(char) == 1`, `sizeof(int) == 2`, `sizeof(long) == 4`. Pointer
arithmetic on `int*` increments by 2, matching the C standard.

**Usable RAM**: Hack has 32K words (64 KB). With int = 2 words, int-heavy
code uses ~2× the RAM, giving roughly 16 KB of effectively usable data space
for general programs. Sufficient for games; tight but workable for a Unix kernel.

### Arithmetic: 8-bit operations with natural carry detection

Each int word holds an 8-bit value (0–255). Arithmetic stays in 8-bit:

```
// Addition of two int bytes — no pack/unpack required
D = lo_a + lo_b          // D is 0–510, fits in 16-bit word
carry = D & 0x0100       // bit 8 set = carry out — one AND instruction
result_lo = D & 0x00FF   // low byte — one AND instruction

D = hi_a + hi_b
D = D + carry_in         // add carry from low
result_hi = D & 0x00FF
overflow = D & 0x0100    // overflow flag if needed
```

No subtraction, no comparison, no sign-bit manipulation. The 16-bit Hack
word naturally provides the 9th bit that makes carry detection trivial.
This is **simpler** than the carry detection formula needed for 16-bit long
arithmetic in the current model.

### Cost impact

| Operation | Current model (int=1 word) | Model B (int=2 words) |
|-----------|---------------------------|----------------------|
| int add/sub | ~13 instr | ~25 instr (~2×) |
| int load | 2 instr | 4 instr |
| int store | 3 instr | 6 instr |
| long add | ~35 instr (complex carry) | ~50 instr (simple carry) |
| Carry detect | sign-bit formula (6 ops) | `& 0x0100` (1 op) |

The `int` overhead is real (~2×) but modest. `long` arithmetic is
comparable between models, and carry detection is actually simpler in Model B.

### Implementation plan

This is a **breaking change** to the entire compiler. Implement as a separate
phase after the Tier 1/2 language features are stable:

1. Change `type_size()` in `sema.rs`: `Int → 2`, `Char → 1`, `Ptr → 1`
2. Add `Type::Long` (4 words) to `parser.rs`; add `KwLong` to `lexer.rs`
3. Add 2-word load/store codegen paths to `codegen.rs`
4. Add 8-bit arithmetic subroutines (`__int_add`, `__int_sub`) with
   `& 0x00FF` carry detection
5. Update calling convention: int args/returns occupy 2 stack words
6. Add `Type::ULong`, `KwUnsigned` support
7. Update all tests (many will break — expected and acceptable)

### POSIX compliance note

With Model B in place, the following POSIX requirements are satisfied:

- `char*` cast of `int*` gives byte-level access ✅
- `sizeof(int) >= sizeof(short) >= sizeof(char)` ✅  
- `sizeof(int) == 2` (matches LP16 / PDP-11 data model used by Unix V6) ✅
- Integer overflow is detectable via the carry bit ✅
- `int` and `unsigned int` have the same representation width ✅

The resulting type model matches the **LP16 data model** used on the original
PDP-11 for which Unix V6 was written, making it the correct target for a
faithful Unix V6 port.

---

## Tier 3 — Hard / Low Priority

### 3.1 Post-increment returns the old value

Current `i++` is implemented as `i += 1` (returns new value, not old).
Fix: before incrementing, push old value, increment, restore old value as
expression result. Requires a temporary variable allocation.

### 3.2 `goto` and labels

Low value, high implementation cost. Not recommended.

### 3.3 `float` / `double`

Not feasible without a software floating-point library. The Hack CPU has
no FP hardware. A 16-bit fixed-point alternative (`int` scaled by 256)
would cover most game physics. Not planned.

### 3.4 Initialiser lists `int arr[] = {1, 2, 3}`

**Where**: parser, sema, codegen  
**Effort**: ~1 hour for simple cases

Only constant initialisers needed (emitted via DataInit).
Parser needs to handle `{expr, expr, ...}` in variable declarations.

### 3.5 `const` qualifier

Parse and ignore for now; add to `VarInfo` for future read-only enforcement.

---

## Recommended Implementation Order

1. Shift operators (`<<` `>>`) — needed for graphics and bit manipulation
2. `do-while` — needed for many game loops
3. `break` / `continue` — needed for any non-trivial loop
4. Compound assignment completion (`*=`, `/=`, etc.)
5. Cast `(type)expr`
6. `switch/case`
7. `typedef`
8. `enum`
9. Array initialisers
10. Multi-dimensional arrays

---

## Notes on Parser Maintainability

The current recursive-descent parser is clean and easy to extend.
Each new precedence level is a new `parse_X_expr()` function.
The current precedence chain from top to bottom is:
```
assign → conditional (ternary) → or → and → bitor → bitxor → bitand
→ eq → rel → add → mul → unary → postfix → primary
```
Shift operators insert between `rel` and `add`.
