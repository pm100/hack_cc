// PROVIDES: strcat
// DEPS: __strcat
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
