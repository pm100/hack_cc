.provides strlen
// DEPS: __strlen
// VM-convention wrapper: strlen(s) -> length of null-terminated string
(strlen)
// ARG[0] = s -> R13
@ARG
A=M
D=M
@R13
M=D
// Call __strlen via R3-convention
@__wrap_strlen_ret
D=A
@R3
M=D
@__strlen
0;JMP
(__wrap_strlen_ret)
// Result is in R13; push it
@R13
D=M
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
