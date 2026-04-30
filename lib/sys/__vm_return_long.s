.provides __vm_return_long
// VM return trampoline for functions returning a 2-word Long value.
// Pops lo (top of stack) and hi, stores to ARG[0]=hi, ARG[1]=lo.
// SP = ARG + 2, then restores THAT, THIS, ARG, LCL, jumps to return address.
(__vm_return_long)
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
// pop lo (top of stack) -> R15
@SP
M=M-1
A=M
D=M
@R15
M=D
// pop hi -> ARG[0]
@SP
M=M-1
A=M
D=M
@ARG
A=M
M=D
// ARG[1] = lo (R15)
@ARG
D=M+1
@R9
M=D
@R15
D=M
@R9
A=M
M=D
// SP = ARG + 2
@ARG
D=M+1
D=D+1
@SP
M=D
// Restore THAT = *(FRAME-1)
@R13
AM=M-1
D=M
@THAT
M=D
// Restore THIS = *(FRAME-2)
@R13
AM=M-1
D=M
@THIS
M=D
// Restore ARG = *(FRAME-3)
@R13
AM=M-1
D=M
@ARG
M=D
// Restore LCL = *(FRAME-4)
@R13
AM=M-1
D=M
@LCL
M=D
// goto retAddr
@R14
A=M
0;JMP
