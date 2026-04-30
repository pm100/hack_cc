.provides __lsub
// 32-bit subtract: R5:R6 - R7:R8 → R5:R6
// Borrow detection for lo_a - lo_b (unsigned lo_a < unsigned lo_b).
// Return via R3.
(__lsub)
// R9 = diff_lo = lo_a - lo_b
@R6
D=M
@R8
D=D-M
@R9
M=D
// Borrow detection
@R6
D=M
@__lsub_la_neg
D;JLT
// lo_a >= 0
@R8
D=M
@__lsub_lb_neg_la_pos
D;JLT
// both >= 0: borrow = (diff_lo < 0)
@R9
D=M
@__lsub_b1
D;JLT
@R10
M=0
@__lsub_have_borrow
0;JMP
(__lsub_b1)
@R10
M=1
@__lsub_have_borrow
0;JMP
(__lsub_lb_neg_la_pos)
// lo_a >= 0, lo_b < 0: always borrow (lo_b unsigned >= 32768 > lo_a)
@R10
M=1
@__lsub_have_borrow
0;JMP
(__lsub_la_neg)
// lo_a < 0
@R8
D=M
@__lsub_lb_pos_la_neg
D;JGE
// both < 0: borrow = (diff_lo < 0)
@R9
D=M
@__lsub_b2
D;JLT
@R10
M=0
@__lsub_have_borrow
0;JMP
(__lsub_b2)
@R10
M=1
@__lsub_have_borrow
0;JMP
(__lsub_lb_pos_la_neg)
// lo_a < 0, lo_b >= 0: never borrow
@R10
M=0
(__lsub_have_borrow)
@R9
D=M
@R6
M=D
@R5
D=M
@R7
D=D-M
@R10
D=D-M
@R5
M=D
@R3
A=M
0;JMP
