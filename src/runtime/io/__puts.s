// PROVIDES: __puts
// DEPS:
// Print null-terminated string at R13 to output port, then newline. Return via R3.
(__puts)
@R13
A=M
D=M
@__puts_end
D;JEQ
@32767
M=D
@R13
M=M+1
@__puts
0;JMP
(__puts_end)
@10
D=A
@32767
M=D
@R3
A=M
0;JMP
