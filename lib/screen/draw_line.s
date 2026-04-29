.provides draw_line
// DEPS: __draw_pixel
// VM-convention: draw_line(x1, y1, x2, y2) -> void.
// ARG[0]=x1, ARG[1]=y1, ARG[2]=x2, ARG[3]=y2.
// Bresenham line algorithm. Scratch: R5-R12.
(draw_line)
// Load args
@ARG
D=M
@R5
M=D
A=D
D=M
@R6
M=D
@R5
D=M+1
A=D
D=M
@R7
M=D
@R5
D=M
@2
D=D+A
A=D
D=M
@R8
M=D
@R5
D=M
@3
D=D+A
A=D
D=M
@R9
M=D
// R6=x1, R7=y1, R8=x2, R9=y2
// dx = abs(x2-x1) in R10
@R8
D=M
@R6
D=D-M
@R10
M=D
@__dl_dx_pos
D;JGE
@R10
M=-M
(__dl_dx_pos)
// dy = abs(y2-y1) in R11
@R9
D=M
@R7
D=D-M
@R11
M=D
@__dl_dy_pos
D;JGE
@R11
M=-M
(__dl_dy_pos)
// sx: R12 = x2>x1 ? 1 : -1
@R8
D=M
@R6
D=D-M
@R12
M=1
@__dl_sx_done
D;JGT
@R12
M=-1
(__dl_sx_done)
// sy: save in ARG[0] slot (R5 already done, reuse R5 as sy)
// Actually use the ARG base address for storage. Use stack push/pop approach.
// Just store sy on stack temporarily
@R9
D=M
@R7
D=D-M
@SP
M=M+1
A=M-1
M=1
@__dl_sy_done
D;JGT
@SP
A=M-1
M=-1
(__dl_sy_done)
// err = dx - dy
@R10
D=M
@R11
D=D-M
// err in a register — we need more regs. Use frame base @R5 to store err.
// Store additional vars just above current SP (we'll manage carefully)
// Store err: push
@SP
M=M+1
A=M-1
M=D
// Stack layout: SP[-1]=err, SP[-2]=sy
(__dl_loop)
// draw_pixel(x1, y1)
@R6
D=M
@R13
M=D
@R7
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
// check if x1==x2 && y1==y2
@R6
D=M
@R8
D=D-M
@__dl_chk_y
D;JNE
@R7
D=M
@R9
D=D-M
@__dl_done
D;JEQ
(__dl_chk_y)
// e2 = 2*err; err is SP[-1]
@SP
A=M-1
D=M
D=D+M
// e2 in D
// save e2 temporarily
@R13
M=D
// if e2 > -dy: err -= dy; x1 += sx
// Check: e2 + dy > 0 (i.e., e2 > -dy)
@R11
D=M
@R13
D=D+M
@__dl_skip_x
D;JLE
// err -= dy
@SP
A=M-1
D=M
@R11
D=D-M
@SP
A=M-1
M=D
// x1 += sx
@R12
D=M
@R6
M=D+M
(__dl_skip_x)
// if e2 < dx: err += dx; y1 += sy
@SP
A=M-1
D=M
D=D+M
@R10
D=M-D
@__dl_skip_y
D;JLE
// err += dx
@SP
A=M-1
D=M
@R10
D=D+M
@SP
A=M-1
M=D
// y1 += sy (SP[-2])
@SP
D=M
@2
A=D-A
D=M
@R7
M=D+M
(__dl_skip_y)
@__dl_loop
0;JMP
(__dl_done)
// Pop temporary stack variables
@SP
M=M-1
@SP
M=M-1
// VM return 0
@LCL
D=M
@R13
M=D
@5
A=D-A
D=M
@R14
M=D
@0
D=A
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
