# Multi-file linking demo

Demonstrates compiling three separate C files into one Hack binary.

## Files

| File | Role |
|------|------|
| `main.c` | Entry point; uses both libraries |
| `vec2.h` / `vec2.c` | 2D integer vector math library |
| `stats.h` / `stats.c` | Array statistics library |

## How to build

```
hack_cc main.c vec2.c stats.c -o multi_demo.asm
hack_emu multi_demo.asm
```

## What happens

Each `.c` file is preprocessed and parsed independently.  The compiler merges
the three `Program` ASTs (deduplicating struct definitions and forward
declarations that appear in multiple files via headers), then runs a single
sema → codegen → linker pass on the merged program.

## Expected output

```
=== vec2 demo ===
add.x: 4
add.y: 2
scale.x: 9
scale.y: 12
dot: -5
manhattan: 14
=== stats demo ===
sum: 150
avg: 25
min: 10
max: 50
count(20): 3
```
