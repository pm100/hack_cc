.provides __strcat
// DEPS:
// Append src (R14) to end of dst (R13). R13 preserved. Scratch: R5. Return via R3.
(__strcat)
@R13
D=M
@R5
M=D
(__strcat_find_end)
@R5
A=M
D=M
@__strcat_copy
D;JEQ
@R5
M=M+1
@__strcat_find_end
0;JMP
(__strcat_copy)
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
@__strcat_copy
D;JNE
@R3
A=M
0;JMP
