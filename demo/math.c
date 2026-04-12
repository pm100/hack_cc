/* math.c — Math operations demo.
 *
 * Exercises: abs, min, max, *, /, % (pulls in __mul and __div).
 * Demonstrates that only math builtins are linked — no screen or string code.
 *
 * Compile: hack_cc math.c
 * Run:     hack_emu math.hack
 */
#include <hack.h>

int factorial(int n) {
    if (n <= 1) return 1;
    return n * factorial(n - 1);
}

int gcd(int a, int b) {
    a = abs(a);
    b = abs(b);
    while (b != 0) {
        int t = b;
        b = a % b;
        a = t;
    }
    return a;
}

int main() {
    char buf[12];

    /* Multiply and divide */
    itoa(6 * 7, buf);        puts(buf);   /* 42 */
    itoa(100 / 7, buf);      puts(buf);   /* 14 */
    itoa(100 % 7, buf);      puts(buf);   /* 2 */

    /* abs, min, max */
    itoa(abs(-99), buf);     puts(buf);   /* 99 */
    itoa(min(3, 7), buf);    puts(buf);   /* 3 */
    itoa(max(3, 7), buf);    puts(buf);   /* 7 */

    /* factorial */
    itoa(factorial(10), buf); puts(buf);  /* 3628800 — overflows 16-bit! */
    itoa(factorial(6), buf);  puts(buf);  /* 720 */

    /* gcd */
    itoa(gcd(48, 18), buf);  puts(buf);  /* 6 */
    itoa(gcd(-35, 14), buf); puts(buf);  /* 7 */

    return 0;
}
