// PROVIDES: puts
// DEPS: __puts
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
// VM return sequence
@LCL
D=M
@R13
M=D
@5
A=D-A
D=M
@R14
M=D
@SP
M=M-1
A=M
D=M
@ARG
A=M
M=D
@ARG
D=M+1
@SP
M=D
@R13
AM=M-1
D=M
@THAT
M=D
@R13
AM=M-1
D=M
@THIS
M=D
@R13
AM=M-1
D=M
@ARG
M=D
@R13
AM=M-1
D=M
@LCL
M=D
@R14
A=M
0;JMP
