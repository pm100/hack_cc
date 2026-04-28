// PROVIDES: __puts_screen
// DEPS: __console_putchar
// Print null-terminated string at R13 to screen console, then newline. Return via R3.
// Uses named vars: __pss_r3 (saved R3), __pss_ptr (current string pointer).
(__puts_screen)
@R3
D=M
@__pss_r3
M=D
@R13
D=M
@__pss_ptr
M=D
(__pss_loop)
// Load next char: D = *ptr
@__pss_ptr
A=M
D=M
@__pss_end
D;JEQ
// Set R13 = char for __console_putchar
@R13
M=D
// Call __console_putchar; ptr is safe in named var __pss_ptr
@__pss_loop_ret
D=A
@R3
M=D
@__console_putchar
0;JMP
(__pss_loop_ret)
// Advance pointer
@__pss_ptr
M=M+1
@__pss_loop
0;JMP
(__pss_end)
// Send trailing newline (char 10)
@10
D=A
@R13
M=D
@__pss_nl_ret
D=A
@R3
M=D
@__console_putchar
0;JMP
(__pss_nl_ret)
@__pss_r3
A=M
0;JMP
