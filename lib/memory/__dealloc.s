.provides __dealloc
// DEPS:
// No-op free (bump allocator). R13 = pointer to free. Return via R3.
(__dealloc)
@R3
A=M
0;JMP
