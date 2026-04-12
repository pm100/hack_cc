// PROVIDES: __strlen
// DEPS:
// Length of null-terminated string at R13. Result in R13. Scratch: R14. Return via R3.
(__strlen)
@R14
M=0
(__strlen_loop)
@R13
A=M
D=M
@__strlen_end
D;JEQ
@R13
M=M+1
@R14
M=M+1
@__strlen_loop
0;JMP
(__strlen_end)
@R14
D=M
@R13
M=D
@R3
A=M
0;JMP
