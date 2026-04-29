.provides srand
// DEPS:
// VM-convention: srand(seed) -> void  (returns 0)
// ARG[0] = seed
// Sets the global __rand_seed used by rand()
(srand)
@ARG
A=M
D=M
@__rand_seed
M=D
// Return 0
@SP
A=M
M=0
@SP
M=M+1
// VM return
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
