# Outstanding issues

1. **File-scope visibility is still enforced too loosely.** `check_scope_flow` feeds `check_sf_expr` a whole-program `known_globals` set, so identifiers can be accepted even when they were never visible at that point in the file or when a block-scope `extern` is already out of scope.

2. **Local `extern` redeclarations still miss some cross-kind/linkage conflicts.** The local declaration checks only look at the current block scope and some function linkage, not conflicting file-scope symbols of the other kind.

3. **`signed`/`unsigned` conflicts are collapsed away too early.** The parser normalizes non-`char` `signed`/`unsigned` types to plain integer types, which makes conflicting redeclarations indistinguishable.

4. **Incomplete struct forward declarations are not parseable.** Top-level parsing handles `struct Name { ... };`, but not standalone `struct Name;`, because typed declarations always expect an identifier after the type.

5. **Member access on returned struct temporaries looks broken in codegen.** `Expr::Member` loads through `gen_addr`, and `gen_addr` only accepts lvalues; expressions like `f().field` or `f().arr[0]` therefore fail despite the current struct-by-value claim.
