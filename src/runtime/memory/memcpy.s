// PROVIDES: memcpy
// DEPS:
// VM-convention: memcpy(dst, src, n) -> dst
// ARG[0]=dst, ARG[1]=src, ARG[2]=n  (n in words)
// Returns: original dst
// Scratch: R13 (dst cursor), R14 (src cursor), R15 (n), R5 (original dst)
(memcpy)
// R13 = dst
@ARG
A=M
D=M
@R13
M=D
// R5 = original dst
@R5
M=D
// R14 = src
@ARG
D=M
A=D+1
D=M
@R14
M=D
// R15 = n
@ARG
D=M
@2
A=D+A
D=M
@R15
M=D
(__memcpy_loop)
@R15
D=M
@__memcpy_done
D;JLE
@R14
A=M
D=M           // D = *src
@R13
A=M
M=D           // *dst = *src
@R13
M=M+1         // dst++
@R14
M=M+1         // src++
@R15
M=M-1         // n--
@__memcpy_loop
0;JMP
(__memcpy_done)
// Return original dst (R5)
@R5
D=M
@SP
A=M
M=D
@SP
M=M+1
// VM return
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
