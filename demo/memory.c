/* memory.c — malloc/free demo.
 *
 * Exercises the bump allocator. Allocates arrays dynamically, fills them,
 * verifies contents, then "frees" (no-op with bump allocator).
 *
 * The linker pulls in malloc, __alloc — and nothing related to screen/string.
 *
 * Compile: hack_cc memory.c
 * Run:     hack_emu memory.hack
 */
#include <hack.h>

/* Fill an int array with consecutive values starting at 'start'. */
void fill_array(int *arr, int n, int start) {
    int i;
    for (i = 0; i < n; i++) {
        arr[i] = start + i;
    }
}

/* Sum the elements of an int array. */
int sum_array(int *arr, int n) {
    int i;
    int s;
    s = 0;
    for (i = 0; i < n; i++) {
        s = s + arr[i];
    }
    return s;
}

int main() {
    char buf[12];
    int *a;
    int *b;
    int *c;
    int s;

    /* Allocate two arrays of 10 ints */
    a = (int *)malloc(10);
    b = (int *)malloc(10);

    /* Fill and verify array a: 0..9, sum = 45 */
    fill_array(a, 10, 0);
    s = sum_array(a, 10);
    itoa(s, buf);
    puts(buf);   /* 45 */

    /* Fill and verify array b: 10..19, sum = 145 */
    fill_array(b, 10, 10);
    s = sum_array(b, 10);
    itoa(s, buf);
    puts(buf);   /* 145 */

    /* Verify the two allocations don't overlap */
    if (a[0] == 0 && b[0] == 10) {
        puts("allocations are separate: ok");
    } else {
        puts("allocation overlap: bug!");
    }

    free(a);
    free(b);

    /* Allocate again — bump allocator advances, old memory is NOT reused */
    c = (int *)malloc(5);
    fill_array(c, 5, 100);
    s = sum_array(c, 5);
    itoa(s, buf);
    puts(buf);   /* 510 */

    return 0;
}
