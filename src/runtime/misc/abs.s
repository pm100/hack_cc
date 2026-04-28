// PROVIDES: abs
// DEPS: (none)
// VM-convention wrapper: abs(x) -> |x|
(abs)
// ARG[0] = x
@ARG
A=M
D=M
// if x >= 0, done; else negate
@__abs_done
D;JGE
D=-D
(__abs_done)
// push return value
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
