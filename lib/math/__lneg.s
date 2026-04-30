.provides __lneg
// Negate: -(R5:R6) → R5:R6
// Algorithm: lo = ~lo+1; if new_lo == 0: hi = ~hi+1 else hi = ~hi
// Return via R3.
(__lneg)
@R6
D=M
D=!D
D=D+1
@R6
M=D
@__lneg_lo_nz
D;JNE
// lo is zero after negation: carry into hi
@R5
D=M
D=!D
D=D+1
@R5
M=D
@R3
A=M
0;JMP
(__lneg_lo_nz)
// no carry: hi = ~hi
@R5
D=M
D=!D
@R5
M=D
@R3
A=M
0;JMP
