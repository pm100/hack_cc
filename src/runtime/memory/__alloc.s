// PROVIDES: __alloc
// DEPS:
// Bump allocator. R13 = number of words to allocate. Returns pointer in R13.
// Heap grows from 2048 upward. __heap_ptr is a global at a fixed RAM address.
// We use RAM[15327] as the heap pointer (just below the font table at 15328).
(__alloc)
@15327
D=M
@__alloc_init
D;JEQ
@__alloc_ready
0;JMP
(__alloc_init)
@2048
D=A
@15327
M=D
(__alloc_ready)
@15327
D=M
@R14
M=D
@R13
D=M
@15327
M=D+M
@R14
D=M
@R13
M=D
@R3
A=M
0;JMP
