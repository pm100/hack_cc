.provides strcmp
// DEPS: __strcmp, __vm_return
// VM-convention wrapper: strcmp(a, b) -> comparison result
// ARG[0]=a->R13, ARG[1]=b->R14; result stored in R13 by __strcmp
(strcmp)
// ARG[0] = a -> R13
@ARG
A=M
D=M
@R13
M=D
// ARG[1] = b -> R14
@ARG
D=M
A=D+1
D=M
@R14
M=D
// Call __strcmp via R3-convention
@__wrap_strcmp_ret
D=A
@R3
M=D
@__strcmp
0;JMP
(__wrap_strcmp_ret)
// Result in R13; push it
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
