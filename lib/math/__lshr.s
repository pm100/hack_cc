.provides __lshr
// Arithmetic right shift 32-bit long: R5:R6 >>= R8 (shift count).
// Computes 2^n in R7:R8, calls __ldiv (truncated), then adjusts for floor
// division by subtracting 1 from quotient when remainder is negative.
// Uses R4 to save caller return address across the __ldiv call.
// Returns via R3.
(__lshr)
    @R8
    D=M
    @__lshr_done
    D;JLE           // count <= 0: no-op
    // Save caller's return address (R4 not used by __ldiv)
    @R3
    D=M
    @R4
    M=D
    // Save shift count in R11 (will be clobbered by __ldiv but used before it)
    @R8
    D=M
    @R11
    M=D
    // Build divisor = 2^count in R7:R8
    @R7
    M=0
    @R8
    M=1
(__lshr_pow_loop)
    @R11
    D=M
    @__lshr_pow_done
    D;JEQ
    @R11
    M=M-1
    // Left shift divisor by 1
    @R8
    D=M
    @__lshr_pow_carry
    D;JLT
    // no carry
    @R7
    D=M
    M=D+M
    @R8
    D=M
    M=D+M
    @__lshr_pow_loop
    0;JMP
(__lshr_pow_carry)
    @R7
    D=M
    M=D+M
    M=M+1
    @R8
    D=M
    M=D+M
    @__lshr_pow_loop
    0;JMP
(__lshr_pow_done)
    // R5:R6 = dividend, R7:R8 = 2^n (divisor)
    @__lshr_after_div
    D=A
    @R3
    M=D
    @__ldiv
    0;JMP
(__lshr_after_div)
    // R5:R6 = quotient (truncated toward 0), R9:R10 = remainder (same sign as dividend)
    // For arithmetic right shift (floor division), if remainder < 0: quotient -= 1
    // A 32-bit value is negative iff its hi word (R9) is negative (signed 16-bit).
    @R9
    D=M
    @__lshr_return
    D;JGE           // remainder >= 0: no adjustment needed
    // remainder < 0: decrement quotient by 1
    @R6
    D=M             // save old lo
    M=M-1           // lo -= 1
    @__lshr_no_borrow
    D;JNE           // old lo != 0: no borrow into hi
    @R5
    M=M-1           // hi -= 1 (borrow)
(__lshr_no_borrow)
(__lshr_return)
    @R4
    A=M
    0;JMP
(__lshr_done)
    @R3
    A=M
    0;JMP
