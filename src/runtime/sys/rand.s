// PROVIDES: rand
// DEPS: __mul
// VM-convention: rand() -> int  (range [0, 32767])
// LCG: seed = seed * 25173 + 13849  (mod 2^16, Hull-Dobell full-period params)
// Returns: new_seed & 0x7FFF  (always non-negative)
// Global state: __rand_seed (initialized to 0 by hardware)
// Scratch: R13 (seed / mul result), R14 (multiplier / retAddr)
(rand)
// Load seed into R13
@__rand_seed
D=M
@R13
M=D
// Load multiplier 25173 into R14
@25173
D=A
@R14
M=D
// Call __mul via R3: R13 = R13 * R14
@__rand_mul_ret
D=A
@R3
M=D
@__mul
0;JMP
(__rand_mul_ret)
// R13 = seed * 25173 (mod 2^16)
// Add 13849
@13849
D=A
@R13
M=D+M         // R13 = seed*25173 + 13849
// Store new seed
@R13
D=M
@__rand_seed
M=D
// Return D & 0x7FFF
@32767
D=D&A
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
