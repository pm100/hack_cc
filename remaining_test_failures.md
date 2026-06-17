# Remaining External Test Failures - Detailed Explanation

All 3 remaining test failures are **switch statement parser limitations** from chapter 8 extra_credit tests.
These are advanced C features that require significant parser restructuring to support.

## Current Test Results

✅ **334 valid tests pass** (99.1% pass rate)
✅ **640 invalid tests correctly rejected** (100% - all semantic checks working)
❌ **3 valid tests fail** (all are chapter_8 extra_credit switch parser bugs)

---

## Bug #1: switch_decl.c - Declaration Before First Case Label

### The Problem
C allows declarations inside a switch body *before* the first case label.
The declaration is in scope, but its initializer is **skipped** when jumping to a case.

### Example Code
```c
int main(void) {
    int a = 3;
    int b = 0;
    switch(a) {
        // Declaration here - BEFORE any case label
        int a = (b = 5);  // This line is in scope but never executed
    case 3:
        a = 4;        // Uses outer 'a' (not the declared one)
        b = b + a;    // b is still 0 (initializer was skipped)
    }
    return a == 3 && b == 4;  // Should return 1 (true)
}
```

### Compiler Error
```
parse error at line 6: expected RBrace, got KwInt
```

### Why It Fails
Our switch parser expects the switch body to immediately contain case/default labels.
When it sees `int a = ...` it tries to parse it as a statement, but the switch
body grammar only allows case labels at the top level.

### Fix Required
Modify `parse_switch_stmt()` to allow declarations/statements before the first
case label. These statements are added to the AST but marked as "unreachable"
or placed in a special "switch header" section that gets skipped during codegen.

---

## Bug #2: switch_nested_cases.c - Case Labels Inside Nested Control Flow

### The Problem
C allows case labels to appear inside nested control structures (if, while, for, etc.).
This is rarely used but legal - the switch can "jump into" the middle of an if statement!

### Example Code
```c
int main(void) {
    int result = 0;
    switch(3) {
        case 0: return 0;
        case 1: if (0) {
            case 3: result = 1; break;  // Case INSIDE the if!
        }
        default: return 0;
    }
    return result;  // Should return 1
}
```

When `switch(3)` executes, it jumps directly to `case 3:`, which is inside
the `if (0)` block. The condition `if (0)` is never evaluated - we jump
straight to the case label!

### Compiler Error
```
parse error at line 6: unexpected token in expression: KwCase
```

### Why It Fails
Our parser only looks for case labels at the **top level** of the switch body.
When parsing the `if` statement, it tries to parse the body, sees `case 3:`
and doesn't know what to do with it (case is not a valid statement keyword
outside of switch context in our grammar).

### Fix Required
Complete parser restructure. The switch parser needs to:
1. Recursively search for case labels inside ALL nested statements
2. Build a jump table that can target any statement, not just top-level ones
3. Handle the complex control flow of jumping into the middle of loops/conditionals

This is the hardest of the three bugs to fix.

---

## Bug #3: duffs_device.c - Duff's Device (Case Labels Inside do-while)

### The Problem
Famous C optimization trick: case labels inside a do-while loop body.
Similar to Bug #2 but specifically for Duff's Device pattern.

### Example Code
```c
int main(void) {
    int count = 37;
    int iterations = (count + 4) / 5;
    switch (count % 5) {  // count % 5 = 2
        case 0:
            do {
                count = count - 1;
                case 4:  // Case labels INSIDE the loop!
                    count = count - 1;
                case 3:
                    count = count - 1;
                case 2:  // Jump here when count % 5 = 2
                    count = count - 1;
                case 1:
                    count = count - 1;
            } while ((iterations = iterations - 1) > 0);
    }
    return (count == 0 && iterations == 0);  // Should return 1
}
```

The first iteration jumps to `case 2:` (inside the loop), then falls through
to `case 1:`, then loops back. Subsequent iterations start at `case 0:`
and execute all 5 decrements. This unrolls a loop in a weird way.

### Compiler Error
```
parse error at line 8: unexpected token in expression: KwCase
```

### Why It Fails
Same root cause as Bug #2: parser doesn't allow case labels inside nested statements.

### Fix Required
Same as Bug #2 - requires full parser restructure to allow case labels anywhere
within the switch body's statement tree.

---

## Summary

All 3 failures are **known C edge cases** that our parser doesn't support:

1. **switch_decl.c** - Easier fix: allow statements before first case
2. **switch_nested_cases.c** - Hard fix: allow cases inside nested control flow
3. **duffs_device.c** - Hard fix: same as #2, specifically for loops

These are rarely-used features (especially #2 and #3). Fixing them would require
significant changes to the switch parsing logic and would NOT improve compiler
functionality for typical C code.

**Recommendation:** Document as known limitations. These features are:
- Rarely used in practice (Duff's Device is a historical curiosity)
- Extra credit tests (not core C functionality)
- Would require substantial parser refactoring for minimal benefit
