// PROVIDES: __sys_wait
// DEPS:
// Approximate busy-wait. R13 = number of milliseconds (approximate).
// Uses a loop count calibrated for ~1ms at Hack CPU speed.
// Scratch: R14. Return via R3.
(__sys_wait)
(__sys_wait_outer)
@R13
D=M
@__sys_wait_done
D;JEQ
@400
D=A
@R14
M=D
(__sys_wait_inner)
@R14
M=M-1
D=M
@__sys_wait_inner
D;JGT
@R13
M=M-1
@__sys_wait_outer
0;JMP
(__sys_wait_done)
@R3
A=M
0;JMP
