.provides strcpy
// DEPS: __strcpy, __vm_return
// VM-convention wrapper: strcpy(dst, src) -> dst pointer
// ARG[0]=dst->R13, ARG[1]=src->R14; __strcpy preserves R13 (original dst)
(strcpy)
// ARG[0] = dst -> R13
@ARG
A=M
D=M
@R13
M=D
// ARG[1] = src -> R14
@ARG
D=M
A=D+1
D=M
@R14
M=D
// Call __strcpy via R3-convention
@__wrap_strcpy_ret
D=A
@R3
M=D
@__strcpy
0;JMP
(__wrap_strcpy_ret)
// Return original dst (R13 preserved by __strcpy)
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
