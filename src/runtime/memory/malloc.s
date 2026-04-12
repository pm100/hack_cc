// PROVIDES: malloc
// DEPS: __alloc
// VM-convention wrapper: malloc(n) -> pointer.
// ARG[0] = n (number of words to allocate). Returns pointer.
// No locals needed.
(malloc)
@ARG
D=M
A=D
D=M
@R13
M=D
@__malloc_alloc_ret
D=A
@R3
M=D
@__alloc
0;JMP
(__malloc_alloc_ret)
// R13 = alloc result. Push it as the return value before the VM return sequence.
@R13
D=M
@SP
A=M
M=D
@SP
M=M+1
// Standard VM return sequence (same as codegen-emitted return).
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
