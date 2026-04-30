.provides __lshl
// Left shift 32-bit long: R5:R6 <<= R8 (shift count).
// Bit-by-bit loop: each step detects carry (bit15 of lo) then doubles both words.
// Returns via R3.
(__lshl)
    @R8
    D=M
    @__lshl_done
    D;JLE           // count <= 0: no-op
(__lshl_loop)
    @R8
    D=M
    @__lshl_done
    D;JEQ
    @R8
    M=M-1           // count--
    @R6
    D=M
    @__lshl_carry
    D;JLT           // lo bit15 set = carry into hi
    // no carry: hi = hi*2, lo = lo*2
    @R5
    D=M
    M=D+M
    @R6
    D=M
    M=D+M
    @__lshl_loop
    0;JMP
(__lshl_carry)
    // carry: hi = hi*2 + 1, lo = lo*2
    @R5
    D=M
    M=D+M
    M=M+1
    @R6
    D=M
    M=D+M
    @__lshl_loop
    0;JMP
(__lshl_done)
    @R3
    A=M
    0;JMP
