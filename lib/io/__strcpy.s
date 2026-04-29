.provides __strcpy
// DEPS:
// Copy null-terminated string from R14 (src) to R13 (dst). R13 preserved. Scratch: R5. Return via R3.
(__strcpy)
@R13
D=M
@R5
M=D
(__strcpy_loop)
@R14
A=M
D=M
@R5
A=M
M=D
@R14
M=M+1
@R5
M=M+1
@__strcpy_loop
D;JNE
@R3
A=M
0;JMP
