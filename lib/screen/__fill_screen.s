.provides __fill_screen
// DEPS:
// Fill entire screen with black (-1). Scratch: R13. Return via R3.
(__fill_screen)
@16384
D=A
@R13
M=D
(__fill_loop)
@24576
D=A
@R13
D=D-M
@__fill_done
D;JLE
@R13
A=M
M=-1
@R13
M=M+1
@__fill_loop
0;JMP
(__fill_done)
@R3
A=M
0;JMP
