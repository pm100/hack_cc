.provides __lmul
// Multiply R5:R6 * R7:R8 → R5:R6 (signed 32-bit)
// Scratch: R9-R15. Return via R3.
// Algorithm: MSB-first shift-and-add, 32 iterations.
(__lmul)
// R12 = sign flag (0 = positive result)
@R12
M=0
// If a (R5:R6) < 0: negate a, toggle sign
@R5
D=M
@__lmul_a_pos
D;JGE
// Negate R5:R6
@R6
D=M
D=!D
D=D+1
@R6
M=D
@__lmul_aneg_lo_nz
D;JNE
@R5
D=M
D=!D
D=D+1
@R5
M=D
@__lmul_a_done
0;JMP
(__lmul_aneg_lo_nz)
@R5
D=M
D=!D
@R5
M=D
(__lmul_a_done)
// Toggle sign: 0 → -1 (all bits set)
@R12
M=!M
(__lmul_a_pos)
// If b (R7:R8) < 0: negate b, toggle sign
@R7
D=M
@__lmul_b_pos
D;JGE
// Negate R7:R8
@R8
D=M
D=!D
D=D+1
@R8
M=D
@__lmul_bneg_lo_nz
D;JNE
@R7
D=M
D=!D
D=D+1
@R7
M=D
@__lmul_b_done
0;JMP
(__lmul_bneg_lo_nz)
@R7
D=M
D=!D
@R7
M=D
(__lmul_b_done)
// Toggle sign
@R12
M=!M
(__lmul_b_pos)
// R9:R10 = result accumulator = 0
@R9
M=0
@R10
M=0
// R11 = loop counter = 32
@32
D=A
@R11
M=D
(__lmul_loop)
@R11
D=M
@__lmul_done
D;JEQ
@R11
M=M-1
// --- result (R9:R10) <<= 1 ---
// Save old R10 in R13
@R10
D=M
@R13
M=D
// R10 = 2 * old_R10
@R10
D=M
M=D+M
// Carry from lo shift = (old_R10 < 0)
@R13
D=M
@__lmul_res_carry
D;JLT
// No carry: R9 = 2 * old_R9
@R9
D=M
M=D+M
@__lmul_check_msb
0;JMP
(__lmul_res_carry)
// Carry: R9 = 2 * old_R9 + 1
@R9
D=M
M=D+M
M=M+1
(__lmul_check_msb)
// If MSB of rhs hi (R7 < 0): result += lhs (R5:R6)
@R7
D=M
@__lmul_no_add
D;JGE
// Inline 32-bit add: R9:R10 += R5:R6
// Save old R10 in R14
@R10
D=M
@R14
M=D
// R10 += R6
@R6
D=M
@R10
M=D+M
// Carry detection for R10 addition
@R14
D=M
@__lmul_lo_a_neg
D;JLT
// old_R10 >= 0
@R6
D=M
@__lmul_mixed1
D;JLT
// both >= 0: no carry
@R5
D=M
@R9
M=D+M
@__lmul_no_add
0;JMP
(__lmul_mixed1)
// old_R10 >= 0, R6 < 0: carry = (R10_new >= 0)
@R10
D=M
@__lmul_carry1
D;JGE
// no carry
@R5
D=M
@R9
M=D+M
@__lmul_no_add
0;JMP
(__lmul_carry1)
@R5
D=M
@R9
D=D+M
D=D+1
@R9
M=D
@__lmul_no_add
0;JMP
(__lmul_lo_a_neg)
// old_R10 < 0
@R6
D=M
@__lmul_both_neg
D;JLT
// old_R10 < 0, R6 >= 0: carry = (R10_new >= 0)
@R10
D=M
@__lmul_carry2
D;JGE
// no carry
@R5
D=M
@R9
M=D+M
@__lmul_no_add
0;JMP
(__lmul_carry2)
@R5
D=M
@R9
D=D+M
D=D+1
@R9
M=D
@__lmul_no_add
0;JMP
(__lmul_both_neg)
// both < 0: always carry
@R5
D=M
@R9
D=D+M
D=D+1
@R9
M=D
(__lmul_no_add)
// --- rhs (R7:R8) <<= 1 ---
@R8
D=M
@R13
M=D
@R8
D=M
M=D+M
@R13
D=M
@__lmul_rhs_carry
D;JLT
@R7
D=M
M=D+M
@__lmul_loop
0;JMP
(__lmul_rhs_carry)
@R7
D=M
M=D+M
M=M+1
@__lmul_loop
0;JMP
(__lmul_done)
// Copy result R9:R10 → R5:R6
@R9
D=M
@R5
M=D
@R10
D=M
@R6
M=D
// Apply sign: if R12 != 0, negate result
@R12
D=M
@__lmul_return
D;JEQ
// Negate R5:R6
@R6
D=M
D=!D
D=D+1
@R6
M=D
@__lmul_neg_lo_nz
D;JNE
@R5
D=M
D=!D
D=D+1
@R5
M=D
@__lmul_return
0;JMP
(__lmul_neg_lo_nz)
@R5
D=M
D=!D
@R5
M=D
(__lmul_return)
@R3
A=M
0;JMP
