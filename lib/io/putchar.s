.provides putchar
// DEPS: __vm_return
// VM-convention wrapper: putchar(c) -> write c to output port @32767, return c
(putchar)
// ARG[0] = c
@ARG
A=M
D=M
// Write to output port
@32767
M=D
// Push return value (D still holds c after M=D)
@SP
A=M
M=D
@SP
M=M+1
// VM return
@__vm_return
0;JMP
