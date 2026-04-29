.provides strchr
// DEPS:
// VM-convention: strchr(s, c) -> char* or 0
// ARG[0]=s, ARG[1]=c
// Returns: pointer to first occurrence of c in s, or 0 if not found
// Scratch: R13 (s cursor), R14 (c), R6 (return value)
(strchr)
// R13 = s
@ARG
A=M
D=M
@R13
M=D
// R14 = c
@ARG
D=M
A=D+1
D=M
@R14
M=D
(__strchr_loop)
@R13
A=M
D=M           // D = *s
@__strchr_null
D;JEQ         // null terminator
// Compare *s with c
@R14
D=D-M         // D = *s - c
@__strchr_match
D;JEQ
// No match: advance
@R13
M=M+1
@__strchr_loop
0;JMP
(__strchr_null)
// *s == 0: match only if c == 0
@R14
D=M
@__strchr_match
D;JEQ
// c != 0, not found
@R6
M=0
@__strchr_ret
0;JMP
(__strchr_match)
@R13
D=M
@R6
M=D           // R6 = pointer to match
(__strchr_ret)
// Push result (R6)
@R6
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
