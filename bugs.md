# Bugs and known issues

## ✅ Fixed

- `sizeof(variable)` — now works (SizeofExpr was already implemented; original report was stale)
- `char msg[4] = "abc"; int x = msg[0];` — was producing wrong result; **fixed** (double-null in global array init corrected)
- `char arr[] = "abc";` — was failing to compile; **fixed** (unsized char array from string literal now works)
- String literal null termination — double `\0` bug **fixed** in sema.rs `eval_global_init`
- All output formats (.asm, .hack, .hackem, .tst) — **done**, tested by compile_link test suite
- Adjacent string literal concatenation (`"foo" "bar"`) — **fixed** in parser
- Escape sequence `\?` — **fixed** in lexer
- `case 'A':` (char literal as switch case value) — **fixed** in parser
- Nested and mixed string/compound initializers for global char arrays (e.g. `char arr[3][4] = {{'a','b','c','d'}, "efgh", "ijk"}`) — **fixed** in sema.rs
- `unsigned char` arrays initialized from string literals — **fixed** in codegen
- Struct compound initializer `struct pair x = {1, 2}` — **fixed** in codegen

---

## ❌ Known remaining failures (from external test suite)

### Switch statement limitations (ch8 extra_credit)

- `switch_decl.c`: declaration inside a switch body *before* the first case label is not allowed by our parser. The C standard permits this (the declaration is in scope but its initializer is skipped). **Needs parser fix.**
- `switch_nested_cases.c` / `duffs_device.c`: case/default labels inside nested control-flow (`if`, `while`, `for`) — Duff's Device style. Our switch parser only looks for case labels at the top level of the switch body. **Needs significant parser restructure.**

### Missing semantic checks (invalid-code not rejected)

- `chapter_9/invalid_declarations/redefine_fun_as_var.c` — compiler silently accepts redefining a function name as a variable
- `chapter_9/invalid_declarations/redefine_var_as_fun.c` — compiler silently accepts calling a variable as a function
- `chapter_9/invalid_declarations/undeclared_fun.c` — compiler silently accepts calls to completely undeclared functions
- `chapter_10/invalid_types/conflicting_function_linkage_2.c` — conflicting `extern` / static function linkage not checked

### Struct operator precedence bug (ch18)

- `chapter_18/valid/.../postfix_precedence.c`: `-array[2].b.inner_arr[1]` returns 0 but expects 1 (should be −11, and the test checks `i == -11` returning 1). Negation applied to the wrong sub-expression due to operator precedence issue with struct member access on array elements. **Needs investigation.**

---

## ℹ️ Notes

- `demo/cal.c` — compiles and appears to run correctly. No automated test; requires interactive keyboard input to verify.
- The external test suite currently shows: **333 pass, 331 skip, 4 fail** (valid) + **636 reject, 4 accepted** (invalid).
