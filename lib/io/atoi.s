.provides atoi
// DEPS:
// VM-convention wrapper: atoi(s) -> int
// ARG[0] = s (pointer to null-terminated decimal string, optional leading '-')
// Returns: integer value
// Scratch: R13 (s ptr), R14 (result), R5 (sign), R6 (temp), R15 (digit)
(atoi)
// R13 = s
@ARG
A=M
D=M
@R13
M=D
// R14 = 0 (result accumulator)
@R14
M=0
// R5 = 0 (sign: 0=positive, 1=negative)
@R5
M=0
// Check for leading '-'
@R13
A=M
D=M
@45
D=D-A
@__atoi_digits
D;JNE
// Is '-': set sign, advance ptr
@R5
M=1
@R13
M=M+1
(__atoi_digits)
@R13
A=M
D=M
@__atoi_done
D;JEQ
// Check D >= '0' (48)
@48
D=D-A
@__atoi_done
D;JLT
// Check D-'0' <= 9
@9
D=D-A
@__atoi_done
D;JGT
// Re-load digit = *s - '0'
@R13
A=M
D=M
@48
D=D-A
@R15
M=D           // R15 = digit
// R14 = R14 * 10 + digit
// Compute using doublings: result*10 = result*8 + result*2
// Save R14*2 first, then double twice more for *8, add R14*2
@R14
D=M
D=D+M         // D = result*2
@R6
M=D           // R6 = result*2
// R14 = result*2
@R14
M=D
// R14 = result*4
@R14
D=M
D=D+M
M=D
// R14 = result*8
@R14
D=M
D=D+M
M=D
// R14 = result*8 + result*2 = result*10
@R6
D=M
@R14
M=D+M
// R14 = result*10 + digit
@R15
D=M
@R14
M=D+M
// s++
@R13
M=M+1
@__atoi_digits
0;JMP
(__atoi_done)
// Apply sign
@R5
D=M
@__atoi_return
D;JEQ
@R14
M=-M
(__atoi_return)
// Push result
@R14
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
