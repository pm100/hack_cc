.provides __draw_line
// DEPS: __draw_pixel
// Bresenham line from (R13=x1, R14=y1) to (R15=x2, stack top before call = y2).
// Because we only have 3 arg registers, caller pushes y2 on stack before jumping.
// Stack protocol: caller pushes y2, then sets R3=return_addr and jumps.
// On entry: R13=x1, R14=y1, R15=x2; SP-1 = y2.
// Scratch: R4,R5,R6,R7,R8,R9,R10,R11,R12.
(__draw_line)
@R3
D=M
@R4
M=D
@SP
M=M-1
A=M
D=M
@R12
M=D
// R12 = y2, R13=x1, R14=y1, R15=x2
// dx = abs(x2-x1), dy = abs(y2-y1)
// sx = x1<x2 ? 1 : -1, sy = y1<y2 ? 1 : -1
// err = dx - dy; loop until x1==x2 && y1==y2
// Use R5=x1, R6=y1, R7=dx, R8=dy, R9=sx, R10=sy, R11=err
@R13
D=M
@R5
M=D
@R14
D=M
@R6
M=D
// dx = x2 - x1
@R15
D=M
@R5
D=D-M
@R7
M=D
@__dl_dx_pos
D;JGE
@R7
M=-M
(__dl_dx_pos)
// dy = y2 - y1
@R12
D=M
@R6
D=D-M
@R8
M=D
@__dl_dy_pos
D;JGE
@R8
M=-M
(__dl_dy_pos)
// sx
@R15
D=M
@R5
D=D-M
@R9
M=1
@__dl_sx_set
D;JGT
@R9
M=-1
(__dl_sx_set)
// sy
@R12
D=M
@R6
D=D-M
@R10
M=1
@__dl_sy_set
D;JGT
@R10
M=-1
(__dl_sy_set)
// err = dx - dy
@R7
D=M
@R8
D=D-M
@R11
M=D
(__dl_loop)
// draw_pixel(x1, y1): R13=x1=R5, R14=y1=R6
@R5
D=M
@R13
M=D
@R6
D=M
@R14
M=D
@__dl_px_ret
D=A
@R3
M=D
@__draw_pixel
0;JMP
(__dl_px_ret)
// check done
@R5
D=M
@R15
D=D-M
@__dl_chk_y
D;JNE
@R6
D=M
@R12
D=D-M
@__dl_done
D;JEQ
(__dl_chk_y)
// e2 = 2 * err
@R11
D=M
D=D+M
// if e2 > -dy: err -= dy, x1 += sx
@R8
D=D-M
@__dl_skip_x
D;JLE
@R8
D=M
@R11
M=M-D
@R10
D=M
// Actually: e2 = 2*err; if e2 > -dy: err -= dy; x += sx
// Redo properly
@R11
D=M
D=D+M
@R8
D=D+M
@__dl_skip_x
D;JLE
@R11
D=M
@R8
D=D-M
@R11
M=D
@R9
D=M
@R5
M=D+M
(__dl_skip_x)
@R11
D=M
D=D+M
@R7
D=D-M
@__dl_skip_y
D;JGE
@R11
D=M
@R7
D=D+M
@R11
M=D
@R10
D=M
@R6
M=D+M
(__dl_skip_y)
@__dl_loop
0;JMP
(__dl_done)
@R4
A=M
0;JMP
