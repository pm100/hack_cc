.provides itoa
// DEPS: __itoa
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
