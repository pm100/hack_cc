.provides getchar
// DEPS: __vm_return
// VM-convention: getchar() -> wait for keypress then release; return keycode
(getchar)
// Spin until key pressed
(__getchar_wait_press)
@KBD
D=M
@__getchar_wait_press
D;JEQ
// Save keycode
@R13
M=D
// Spin until key released
(__getchar_wait_release)
@KBD
D=M
@__getchar_wait_release
D;JNE
// Push return value = saved keycode
@R13
D=M
@SP
A=M
M=D
@SP
M=M+1
// VM return
@__vm_return
0;JMP
