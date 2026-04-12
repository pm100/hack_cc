// PROVIDES: __strcmp
// DEPS:
// Compare strings at R13 (a) and R14 (b). Result (*a-*b at first diff, 0 if equal) in R13.
// Scratch: R6. Return via R3.
(__strcmp)
(__strcmp_loop)
@R13
A=M
D=M
@R6
M=D
@R14
A=M
D=M
@R6
D=M-D
@__strcmp_ne
D;JNE
@R13
A=M
D=M
@__strcmp_done
D;JEQ
@R13
M=M+1
@R14
M=M+1
@__strcmp_loop
0;JMP
(__strcmp_ne)
(__strcmp_done)
@R13
M=D
@R3
A=M
0;JMP
