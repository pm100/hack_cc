.provides __vm_call
// VM call trampoline.  Entry: D=retAddr, R13=nArgs, R14=callee_addr
(__vm_call)
// push retAddr (D)
@SP
A=M
M=D
@SP
M=M+1
// push LCL
@LCL
D=M
@SP
A=M
M=D
@SP
M=M+1
// push ARG
@ARG
D=M
@SP
A=M
M=D
@SP
M=M+1
// push THIS
@THIS
D=M
@SP
A=M
M=D
@SP
M=M+1
// push THAT
@THAT
D=M
@SP
A=M
M=D
@SP
M=M+1
// ARG = SP - R13 - 5
@SP
D=M
@5
D=D-A
@R13
D=D-M
@ARG
M=D
// LCL = SP
@SP
D=M
@LCL
M=D
// goto callee (ROM address in R14)
@R14
A=M
0;JMP
