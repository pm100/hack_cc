.provides read_key
// DEPS: __vm_return
// VM-convention wrapper: read_key() -> current keyboard port value (non-blocking)
(read_key)
@KBD
D=M
// push return value
@SP
A=M
M=D
@SP
M=M+1
// VM return
@__vm_return
0;JMP
