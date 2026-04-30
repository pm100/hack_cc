.provides __ldiv
// Signed 32-bit division: R5:R6 / R7:R8 -> quotient R5:R6, remainder R9:R10
// Scratch: R11-R15. Return via R3.
// Algorithm: binary long division (32 iterations, MSB-first).
(__ldiv)
// R12 = quotient sign flag (0=positive)
// R13 = remainder sign flag (0=positive)
@R12
M=0
@R13
M=0
// Handle dividend sign (R5:R6)
@R5
D=M
@__ldiv_div_pos
D;JGE
// dividend < 0: negate R5:R6
@R6
D=M
D=!D
D=D+1
@R6
M=D
@__ldiv_dneg_lo_nz
D;JNE
@R5
D=M
D=!D
D=D+1
@R5
M=D
@__ldiv_dneg_done
0;JMP
(__ldiv_dneg_lo_nz)
@R5
D=M
D=!D
@R5
M=D
(__ldiv_dneg_done)
// Toggle both sign flags
@R12
M=!M
@R13
M=!M
(__ldiv_div_pos)
// Handle divisor sign (R7:R8)
@R7
D=M
@__ldiv_dvs_pos
D;JGE
// divisor < 0: negate R7:R8, toggle quotient sign only
@R8
D=M
D=!D
D=D+1
@R8
M=D
@__ldiv_dvsneg_lo_nz
D;JNE
@R7
D=M
D=!D
D=D+1
@R7
M=D
@__ldiv_dvsneg_done
0;JMP
(__ldiv_dvsneg_lo_nz)
@R7
D=M
D=!D
@R7
M=D
(__ldiv_dvsneg_done)
@R12
M=!M
(__ldiv_dvs_pos)
// R9:R10 = partial remainder = 0
@R9
M=0
@R10
M=0
// R11 = loop counter = 32
@32
D=A
@R11
M=D
(__ldiv_loop)
@R11
D=M
@__ldiv_finish
D;JEQ
@R11
M=M-1
// Step A: R14 = MSB of dividend hi (R5): 1 if R5 < 0 else 0
@R5
D=M
@__ldiv_msb_set
D;JLT
@R14
M=0
@__ldiv_msb_done
0;JMP
(__ldiv_msb_set)
@R14
M=1
(__ldiv_msb_done)
// Step B: R15 = carry from rem lo (R10) to rem hi (R9): 1 if R10 < 0 else 0
@R10
D=M
@__ldiv_rc_set
D;JLT
@R15
M=0
@__ldiv_rc_done
0;JMP
(__ldiv_rc_set)
@R15
M=1
(__ldiv_rc_done)
// Step C: R9 <<= 1, then R9 += R15 (carry from R10)
@R9
D=M
M=D+M
@R15
D=M
@R9
M=D+M
// Step D: R10 <<= 1, then R10 += R14 (MSB of dividend)
@R10
D=M
M=D+M
@R14
D=M
@R10
M=D+M
// Step E: R15 = carry from dividend lo (R6) to dividend hi (R5)
@R6
D=M
@__ldiv_dc_set
D;JLT
@R15
M=0
@__ldiv_dc_done
0;JMP
(__ldiv_dc_set)
@R15
M=1
(__ldiv_dc_done)
// Step F: R5 <<= 1, R5 += R15
@R5
D=M
M=D+M
@R15
D=M
@R5
M=D+M
// Step G: R6 <<= 1 (new LSB = 0, quotient bit placed here if remainder >= divisor)
@R6
D=M
M=D+M
// Step H: Compare R9:R10 >= R7:R8 (unsigned 32-bit)
// Compare hi: R9 vs R7 (both >= 0 as signed 16-bit after normalization)
@R9
D=M
@R7
D=D-M
@__ldiv_rem_gt
D;JGT
@__ldiv_rem_lt
D;JLT
// R9 == R7: compare lo words unsigned (R10 vs R8)
@R10
D=M
@__ldiv_lo_r10_neg
D;JLT
// R10 >= 0
@R8
D=M
@__ldiv_lo_r8_neg
D;JLT
// Both >= 0: signed = unsigned comparison
@R10
D=M
@R8
D=D-M
@__ldiv_rem_gte
D;JGE
@__ldiv_rem_lt
0;JMP
(__ldiv_lo_r8_neg)
// R10 >= 0, R8 < 0 (unsigned R8 >= 32768 > R10): R10 < R8
@__ldiv_rem_lt
0;JMP
(__ldiv_lo_r10_neg)
// R10 < 0
@R8
D=M
@__ldiv_lo_both_neg
D;JLT
// R10 < 0, R8 >= 0 (unsigned R10 >= 32768 > R8): R10 > R8
@__ldiv_rem_gt
0;JMP
(__ldiv_lo_both_neg)
// Both < 0: signed comparison = unsigned comparison
@R10
D=M
@R8
D=D-M
@__ldiv_rem_gte
D;JGE
@__ldiv_rem_lt
0;JMP
(__ldiv_rem_gt)
(__ldiv_rem_gte)
// Remainder >= divisor: subtract divisor from remainder, set quotient bit
// Compute R10 - R8, detect borrow
@R10
D=M
@R8
D=D-M
@R14
M=D
// Borrow detection for R10 - R8
@R10
D=M
@__ldiv_sub_r10_neg
D;JLT
// R10 >= 0
@R8
D=M
@__ldiv_sub_r8_neg_r10_pos
D;JLT
// Both >= 0: borrow = (diff < 0)
@R14
D=M
@__ldiv_sub_borrow
D;JLT
// no borrow
@R15
M=0
@__ldiv_sub_borrow_done
0;JMP
(__ldiv_sub_borrow)
@R15
M=1
@__ldiv_sub_borrow_done
0;JMP
(__ldiv_sub_r8_neg_r10_pos)
// R10 >= 0, R8 < 0: always borrow
@R15
M=1
@__ldiv_sub_borrow_done
0;JMP
(__ldiv_sub_r10_neg)
// R10 < 0
@R8
D=M
@__ldiv_sub_both_neg
D;JLT
// R10 < 0, R8 >= 0: never borrow
@R15
M=0
@__ldiv_sub_borrow_done
0;JMP
(__ldiv_sub_both_neg)
// Both < 0: borrow = (diff < 0)
@R14
D=M
@__ldiv_sub_borrow2
D;JLT
@R15
M=0
@__ldiv_sub_borrow_done
0;JMP
(__ldiv_sub_borrow2)
@R15
M=1
(__ldiv_sub_borrow_done)
// Store R10 - R8 result
@R14
D=M
@R10
M=D
// R9 -= R7 + borrow (R15)
@R9
D=M
@R7
D=D-M
@R15
D=D-M
@R9
M=D
// Set quotient LSB: R6 |= 1 (LSB was 0 after left shift)
@R6
M=M+1
@__ldiv_loop
0;JMP
(__ldiv_rem_lt)
// Remainder < divisor: quotient bit stays 0
@__ldiv_loop
0;JMP
(__ldiv_finish)
// R5:R6 = quotient, R9:R10 = remainder
// Apply quotient sign (R12)
@R12
D=M
@__ldiv_quot_done
D;JEQ
// Negate quotient R5:R6
@R6
D=M
D=!D
D=D+1
@R6
M=D
@__ldiv_qneg_lo_nz
D;JNE
@R5
D=M
D=!D
D=D+1
@R5
M=D
@__ldiv_quot_done
0;JMP
(__ldiv_qneg_lo_nz)
@R5
D=M
D=!D
@R5
M=D
(__ldiv_quot_done)
// Apply remainder sign (R13)
@R13
D=M
@__ldiv_rem_done
D;JEQ
// Negate remainder R9:R10
@R10
D=M
D=!D
D=D+1
@R10
M=D
@__ldiv_rneg_lo_nz
D;JNE
@R9
D=M
D=!D
D=D+1
@R9
M=D
@__ldiv_rem_done
0;JMP
(__ldiv_rneg_lo_nz)
@R9
D=M
D=!D
@R9
M=D
(__ldiv_rem_done)
@R3
A=M
0;JMP
