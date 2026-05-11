.provides puts_screen
// DEPS: __puts_screen, __vm_return
// VM-convention wrapper: puts_screen(s) -> print string + newline to screen, return 0.
(puts_screen)
// ARG[0] = s -> R13
@ARG
A=M
D=M
@R13
M=D
// Call __puts_screen via R3-convention
@__wrap_puts_screen_ret
D=A
@R3
M=D
@__puts_screen
0;JMP
(__wrap_puts_screen_ret)
// Push return value = 0
D=0
@SP
A=M
M=D
@SP
M=M+1
// VM return
@__vm_return
0;JMP
