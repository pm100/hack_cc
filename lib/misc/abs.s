.provides abs
// DEPS: __vm_return
// VM-convention wrapper: abs(x) -> |x|
(abs)
// ARG[0] = x
@ARG
A=M
D=M
// if x >= 0, done; else negate
@__abs_done
D;JGE
D=-D
(__abs_done)
// push return value
@SP
A=M
M=D
@SP
M=M+1
// VM return
@__vm_return
0;JMP
