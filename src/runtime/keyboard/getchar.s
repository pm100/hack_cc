// PROVIDES: getchar
// DEPS: (none)
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
// VM return sequence
@LCL
D=M
@R13
M=D
@5
A=D-A
D=M
@R14
M=D
@SP
M=M-1
A=M
D=M
@ARG
A=M
M=D
@ARG
D=M+1
@SP
M=D
@R13
AM=M-1
D=M
@THAT
M=D
@R13
AM=M-1
D=M
@THIS
M=D
@R13
AM=M-1
D=M
@ARG
M=D
@R13
AM=M-1
D=M
@LCL
M=D
@R14
A=M
0;JMP
