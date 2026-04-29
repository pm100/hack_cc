# Agent Instructions

## Committing and pushing

**Do not commit or push unless the user explicitly says to commit.**

Always wait for the user to say "commit", "commit and push", or equivalent before running `git commit` or `git push`.

## Build and test

- Build: `cargo build`
- Test: `cargo test` (runs 99 tests total: 27 assembler/linker unit + 71 emulator unit + 1 external suite)
- All tests must pass before any commit.

## Compiling demo programs

```
cargo run --bin hack_cc -- -I include demo/<name>.c -o demo/<name>.hackem
```

## Key conventions

- Runtime library functions must be registered in **both** `src/linker.rs` and `src/sema.rs` (`KNOWN_EXTERNALS` list).
- Public API declarations go in `include/hack.h`.
- Runtime assembly files live in `src/runtime/` (subdirs: `io/`, `memory/`, `screen/`, `keyboard/`, `sys/`, `math/`).
- Hack keyboard keycodes: Enter=128, Backspace=129, Left=130, Up=131, Right=132, Down=133.
- Global variable initializers must be integer constants (string literals not supported as global initializers).
- Use `char *name = "..."` for string pointers — but only in local scope or as function return values; global `char *` initialized from a string literal is not supported.
