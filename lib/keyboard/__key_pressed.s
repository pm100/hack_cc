.provides __key_pressed
// DEPS:
// Read current key from keyboard port (RAM[24576]). Result in R13. Return via R3.
(__key_pressed)
@24576
D=M
@R13
M=D
@R3
A=M
0;JMP
