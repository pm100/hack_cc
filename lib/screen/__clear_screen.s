.provides __clear_screen
// DEPS:
// Fill entire screen with white (0). Scratch: R13. Return via R3.
(__clear_screen)
@16384
D=A
@R13
M=D
(__clrscr_loop)
@24576
D=A
@R13
D=D-M
@__clrscr_done
D;JLE
@R13
A=M
M=0
@R13
M=M+1
@__clrscr_loop
0;JMP
(__clrscr_done)
@R3
A=M
0;JMP
