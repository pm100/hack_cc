.provides strcat
// DEPS: __strcat, __vm_return
// VM-convention wrapper: strcat(dst, src) -> dst pointer
// ARG[0]=dst->R13, ARG[1]=src->R14; __strcat preserves R13 (original dst)
(strcat)
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
// Call __strcat via R3-convention
@__wrap_strcat_ret
D=A
@R3
M=D
@__strcat
0;JMP
(__wrap_strcat_ret)
// Return original dst (R13 preserved by __strcat)
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
