// PROVIDES: putchar_screen
// DEPS: __console_putchar
// VM-convention wrapper: putchar_screen(c) -> write c to screen console, return c.
// Use putchar_screen instead of putchar when targeting a standard emulator
// that does not support the output port (RAM[32767]).
(putchar_screen)
// Load c = ARG[0] into R13 for __console_putchar
@ARG
A=M
D=M
@R13
M=D
// Call __console_putchar (R3-convention: R3=ret addr, R13=char)
@__pc_scr_ret
D=A
@R3
M=D
@__console_putchar
0;JMP
(__pc_scr_ret)
// Push return value c (reload from ARG[0])
@ARG
A=M
D=M
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
