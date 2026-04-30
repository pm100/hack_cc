.provides __ladd
// 32-bit add: R5:R6 + R7:R8 → R5:R6 (R5=hi_a, R6=lo_a, R7=hi_b, R8=lo_b)
// Carry detection for lo_a + lo_b.
// Return via R3.
(__ladd)
// R9 = sum_lo = lo_a + lo_b
@R6
D=M
@R8
D=D+M
@R9
M=D
// Carry detection
@R6
D=M
@__ladd_la_neg
D;JLT
// lo_a >= 0
@R8
D=M
@__ladd_lb_neg_la_pos
D;JLT
// both >= 0: carry = 0
@R10
M=0
@__ladd_have_carry
0;JMP
(__ladd_lb_neg_la_pos)
// lo_a >= 0, lo_b < 0: carry = (sum_lo >= 0)
@R9
D=M
@__ladd_carry0
D;JGE
@R10
M=0
@__ladd_have_carry
0;JMP
(__ladd_carry0)
@R10
M=1
@__ladd_have_carry
0;JMP
(__ladd_la_neg)
// lo_a < 0
@R8
D=M
@__ladd_lb_pos_la_neg
D;JGE
// both < 0: carry = 1
@R10
M=1
@__ladd_have_carry
0;JMP
(__ladd_lb_pos_la_neg)
// lo_a < 0, lo_b >= 0: carry = (sum_lo >= 0)
@R9
D=M
@__ladd_carry1
D;JGE
@R10
M=0
@__ladd_have_carry
0;JMP
(__ladd_carry1)
@R10
M=1
(__ladd_have_carry)
// Store results
@R9
D=M
@R6
M=D
@R5
D=M
@R7
D=D+M
@R10
D=D+M
@R5
M=D
@R3
A=M
0;JMP
