// PROVIDES: __console_putchar
// DEPS: __draw_char
// Write character R13 to screen text console. Advance cursor. Return via R3.
// Special chars: 10 (LF) -> col=0, advance row (with scroll if needed).
//                13 (CR) -> col=0 only.
// Console state stored in named variables: __con_col (0-63), __con_row (0-22).
// Screen layout: 64 text columns, 23 text rows, 8px wide x 11px tall cells.
// Scratch variables: __con_r3, __con_ch, __con_src, __con_dst, __con_n.
(__console_putchar)
// Save return address
@R3
D=M
@__con_r3
M=D
// Check for Backspace (8)
@R13
D=M
@8
D=D-A
@__con_bs
D;JEQ
// Check for LF (10)
@R13
D=M
@10
D=D-A
@__con_lf
D;JEQ
// Check for CR (13)
@R13
D=M
@13
D=D-A
@__con_cr
D;JEQ
// Normal char: draw at current (col, row).
// Save char in __con_ch; R13 will be repurposed as col arg for __draw_char.
@R13
D=M
@__con_ch
M=D
// Set up __draw_char args: R13=col, R14=row, R15=char
@__con_col
D=M
@R13
M=D
@__con_row
D=M
@R14
M=D
@__con_ch
D=M
@R15
M=D
// Call __draw_char via R3-convention
@__con_dc_ret
D=A
@R3
M=D
@__draw_char
0;JMP
(__con_dc_ret)
// Advance column
@__con_col
M=M+1
D=M
@64
D=D-A
@__con_col_wrap
D;JEQ
@__con_done
0;JMP
(__con_col_wrap)
// Column reached 64: wrap to 0, advance row
@__con_col
M=0
// fall through to row advance
(__con_row_adv)
@__con_row
M=M+1
D=M
@23
D=D-A
@__con_scroll
D;JEQ
@__con_done
0;JMP
(__con_lf)
// LF: col=0, advance row
@__con_col
M=0
@__con_row_adv
0;JMP
(__con_cr)
// CR: col=0 only
@__con_col
M=0
@__con_done
0;JMP
(__con_scroll)
// Scroll screen up by one text row (352 words per row).
// Copy 7744 words from screen+352 (16736) to screen+0 (16384).
@16736
D=A
@__con_src
M=D
@16384
D=A
@__con_dst
M=D
@7744
D=A
@__con_n
M=D
(__con_scrl)
@__con_n
D=M
@__con_scrl_done
D;JEQ
@__con_src
A=M
D=M
@__con_dst
A=M
M=D
@__con_src
M=M+1
@__con_dst
M=M+1
@__con_n
M=M-1
@__con_scrl
0;JMP
(__con_scrl_done)
// Clear last text row: 352 words at screen+7744 (addr 24128).
@24128
D=A
@__con_dst
M=D
@352
D=A
@__con_n
M=D
(__con_clr)
@__con_n
D=M
@__con_clr_done
D;JEQ
@__con_dst
A=M
M=0
@__con_dst
M=M+1
@__con_n
M=M-1
@__con_clr
0;JMP
(__con_clr_done)
// Set row=22 (last row), col=0
@22
D=A
@__con_row
M=D
@__con_col
M=0
@__con_done
0;JMP
(__con_bs)
// Backspace: move cursor back one col, draw space to erase
@__con_col
D=M
@__con_bs_ok
D;JGT
// Already at col 0: do nothing
@__con_done
0;JMP
(__con_bs_ok)
@__con_col
M=M-1
// Draw a space (32) at the new cursor position to erase
@__con_col
D=M
@R13
M=D
@__con_row
D=M
@R14
M=D
@32
D=A
@R15
M=D
@__con_bs_ret
D=A
@R3
M=D
@__draw_char
0;JMP
(__con_bs_ret)
@__con_done
0;JMP
(__con_done)
@__con_r3
A=M
0;JMP
