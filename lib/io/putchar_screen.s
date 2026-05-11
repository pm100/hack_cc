.provides putchar_screen
// DEPS: __console_putchar, __vm_return
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
// VM return
@__vm_return
0;JMP
