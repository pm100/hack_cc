.provides puts
// DEPS: __puts, __vm_return
// VM-convention wrapper: puts(s) -> print null-terminated string + newline, return 0
(puts)
// ARG[0] = s -> R13
@ARG
A=M
D=M
@R13
M=D
// Call __puts via R3-convention
@__wrap_puts_ret
D=A
@R3
M=D
@__puts
0;JMP
(__wrap_puts_ret)
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
