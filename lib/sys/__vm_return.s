.provides __vm_return
// VM return trampoline.
(__vm_return)
// FRAME(R13) = LCL
@LCL
D=M
@R13
M=D
// RET(R14) = *(FRAME-5)
@5
A=D-A
D=M
@R14
M=D
// *ARG = retval (top of stack)
@SP
M=M-1
A=M
D=M
@ARG
A=M
M=D
// SP = ARG + 1
@ARG
D=M+1
@SP
M=D
// THAT = *(FRAME-1)
@R13
AM=M-1
D=M
@THAT
M=D
// THIS = *(FRAME-2)
@R13
AM=M-1
D=M
@THIS
M=D
// ARG = *(FRAME-3)
@R13
AM=M-1
D=M
@ARG
M=D
// LCL = *(FRAME-4)
@R13
AM=M-1
D=M
@LCL
M=D
// goto retAddr
@R14
A=M
0;JMP
