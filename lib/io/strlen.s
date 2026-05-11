.provides strlen
// DEPS: __strlen, __vm_return
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
// VM return
@__vm_return
0;JMP
