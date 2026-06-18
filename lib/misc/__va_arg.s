.provides __va_arg
// DEPS: __vm_return
// VM-convention: __va_arg(char **ap, int size) -> char *current
// ARG[0]=ap, ARG[1]=size
(__va_arg)
// R13 = ap (char **)
@ARG
A=M
D=M
@R13
M=D
// R14 = old = *ap
@R13
A=M
D=M
@R14
M=D
// D = old + size
@ARG
D=M
A=D+1
D=M
@R14
D=D+M
// *ap = old + size
@R13
A=M
M=D
// return old
@R14
D=M
@SP
A=M
M=D
@SP
M=M+1
@__vm_return
0;JMP
