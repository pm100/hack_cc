# Agent Instructions

## Committing and pushing

**CRITICAL: Do NOT commit or push unless the user EXPLICITLY says "commit" or "commit and push" in the current message.**

- Never commit speculatively, never commit "while you're at it", never commit as a final step of a task.
- Do not commit just because work is finished or tests pass.
- Summarized or old conversation history saying "commit" does NOT count — only the user's current message triggers a commit.
- Always wait for the user to type "commit", "commit and push", or equivalent in their current message before running `git commit` or `git push`.

## Build and test

- Build: `cargo build`
- Test: `cargo test` (runs 100 tests total: 27 assembler/linker unit + 71 emulator unit + 1 external suite)
- All tests must pass before any commit.

## Compiling demo programs

```
cargo run --bin hack_cc -- -I include demo/<name>.c -o demo/<name>.hackem
```

## Key conventions

- Runtime library `.s` files live in `lib/` (subdirs: `io/`, `memory/`, `screen/`, `keyboard/`, `sys/`, `math/`, `misc/`).
- Each runtime `.s` file must have `// PROVIDES: symbol_name` on its **first line** — this is how the linker discovers it automatically.
- To add a new runtime function: create a `.s` file in the appropriate `lib/` subdir with `// PROVIDES:` on line 1, and add its declaration to `include/hack.h`. No registration in linker.rs is needed.
- Library discovery: `HACK_LIB` env var > `./lib/` relative to cwd > executable-adjacent `lib/`.
  Use `-L <dir>` flag with `hack_cc` or `hack_ld` to specify explicitly.
- `hack_cc -c` emits `.s` object files with `// PROVIDES:`, `// DATA:`, and `// NEXT_VAR:` metadata.
- `hack_ld` accepts those `.s` files and links them with the runtime library.
- Public API declarations go in `include/hack.h`.
- Hack keyboard keycodes: Enter=128, Backspace=129, Left=130, Up=131, Right=132, Down=133.
- Global variable initializers must be integer constants (string literals not supported as global initializers).
- Use `char *name = "..."` for string pointers — but only in local scope or as function return values; global `char *` initialized from a string literal is not supported.
