.provides memset
// DEPS:
// VM-convention: memset(ptr, val, n) -> ptr
// ARG[0]=ptr, ARG[1]=val, ARG[2]=n  (n in words, not bytes)
// Returns: original ptr
// Scratch: R13 (cursor), R14 (val), R15 (n), R5 (original ptr)
(memset)
// R13 = ptr (cursor)
@ARG
A=M
D=M
@R13
M=D
// R5 = original ptr (saved for return)
@R5
M=D
// R14 = val
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
(__memset_loop)
@R15
D=M
@__memset_done
D;JLE
@R14
D=M
@R13
A=M
M=D           // *ptr = val
@R13
M=M+1         // ptr++
@R15
M=M-1         // n--
@__memset_loop
0;JMP
(__memset_done)
// Return original ptr (R5)
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
