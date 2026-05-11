.provides itoa
// DEPS: __itoa, __vm_return
// VM-convention wrapper: itoa(n, buf) -> buf pointer
// ARG[0]=n->R13, ARG[1]=buf->R14; __itoa restores R14 to buf start on return
(itoa)
// ARG[0] = n -> R13
@ARG
A=M
D=M
@R13
M=D
// ARG[1] = buf -> R14
@ARG
D=M
A=D+1
D=M
@R14
M=D
// Call __itoa via R3-convention
@__wrap_itoa_ret
D=A
@R3
M=D
@__itoa
0;JMP
(__wrap_itoa_ret)
// Return buf start (R14 restored by __itoa)
@R14
D=M
@SP
A=M
M=D
@SP
M=M+1
// VM return
@__vm_return
0;JMP
