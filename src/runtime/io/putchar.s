// PROVIDES: putchar
// DEPS: (none)
// VM-convention wrapper: putchar(c) -> write c to output port @32767, return c
(putchar)
// ARG[0] = c
@ARG
A=M
D=M
// Write to output port
@32767
M=D
// Push return value (D still holds c after M=D)
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
