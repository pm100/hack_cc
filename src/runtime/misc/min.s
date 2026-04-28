// PROVIDES: min
// DEPS: (none)
// VM-convention wrapper: min(a, b) -> smaller of a and b
(min)
// ARG[0] = a -> R13, ARG[1] = b -> R14
@ARG
A=M
D=M
@R13
M=D
@ARG
D=M
A=D+1
D=M
@R14
M=D
// if a <= b return a, else return b
@R13
D=M
@R14
D=D-M
@__min_use_b
D;JGT
// a <= b: return a = (a-b) + b = D + R14
@R14
D=D+M
@__min_done
0;JMP
(__min_use_b)
@R14
D=M
(__min_done)
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
