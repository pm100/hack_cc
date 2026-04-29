/*
 * cal.c — Unix V7 cal ported to hack_cc
 *
 * Original source: Research Unix V7 (Bell Labs, ~1979)
 * Algorithm: Gregorian calendar with historical 1752 British switchover.
 *
 * Usage: run the program; enter month (1-12) and year (1-9999) when prompted.
 * Enter month 0 to quit.
 *
 * Key codes: digits are ASCII, Enter=128, Backspace=129.
 */
//#define HACK_OUTPUT_SCREEN
#include <hack.h>


/* Calendar grid: 6 rows x 24 cols for one month */
char string[144];

/* ------------------------------------------------------------------ */
/* Output helpers                                                       */
/* ------------------------------------------------------------------ */

void print_str(char *s) {
    while (*s)
        putchar(*s++);
}

void print_int(int n) {
    char buf[8];
    itoa(n, buf);
    print_str(buf);
}

/* ------------------------------------------------------------------ */
/* Month name                                                           */
/* ------------------------------------------------------------------ */

char *month_name(int m) {
    if (m == 1)  return "January";
    if (m == 2)  return "February";
    if (m == 3)  return "March";
    if (m == 4)  return "April";
    if (m == 5)  return "May";
    if (m == 6)  return "June";
    if (m == 7)  return "July";
    if (m == 8)  return "August";
    if (m == 9)  return "September";
    if (m == 10) return "October";
    if (m == 11) return "November";
    if (m == 12) return "December";
    return "???";
}

/* ------------------------------------------------------------------ */
/* Input: read a non-negative integer from the keyboard.               */
/* Accepts digits only; Enter (128) terminates; Backspace (129)        */
/* deletes last digit.  Returns -1 if no digits were entered.          */
/* ------------------------------------------------------------------ */

int readint(void) {
    int digits[6];   /* up to 5 digits + sentinel */
    int count;
    int c;
    int i;
    int result;

    count = 0;

    while (1) {
        c = getchar();

        if (c == 128) {          /* Enter */
            putchar(10);
            break;
        }

        if (c == 129) {          /* Backspace */
            if (count > 0) {
                count--;
                putchar(8);      /* BS */
                putchar(' ');
                putchar(8);      /* move cursor back */
            }
            continue;
        }

        if (c >= '0' && c <= '9') {
            if (count < 5) {
                digits[count] = c - '0';
                count++;
                putchar(c);      /* echo */
            }
            continue;
        }

        /* ignore any other key */
    }

    if (count == 0)
        return -1;

    result = 0;
    for (i = 0; i < count; i++)
        result = result * 10 + digits[i];
    return result;
}

/* ------------------------------------------------------------------ */
/* pstr: convert NUL bytes to spaces, trim trailing spaces, print line */
/* ------------------------------------------------------------------ */

void pstr(char *str, int n) {
    int i;
    int last;

    /* Convert NUL -> space */
    for (i = 0; i < n; i++) {
        if (str[i] == 0)
            str[i] = ' ';
    }

    /* Find last non-space */
    last = 0;
    for (i = 0; i < n; i++) {
        if (str[i] != ' ')
            last = i + 1;
    }
    str[last] = 0;

    puts(str);   /* puts adds newline */
}

/* ------------------------------------------------------------------ */
/* jan1: day of week of January 1 for given year.                      */
/* 0=Sunday.  Handles Gregorian calendar + 1752 British switchover.    */
/* ------------------------------------------------------------------ */

int jan1(int yr) {
    int y;
    int d;

    y = yr;
    d = 4 + y + (y + 3) / 4;

    if (y > 1800) {
        d = d - (y - 1701) / 100;
        d = d + (y - 1601) / 400;
    }
    if (y > 1752)
        d = d + 3;

    return d % 7;
}

/* ------------------------------------------------------------------ */
/* cal: fill buffer p (row stride w) with calendar for month m, year y */
/* Each day slot is 3 chars: [tens-or-space][units][space].            */
/* ------------------------------------------------------------------ */

void cal(int m, int y, char *p, int w) {
    int mon[13];
    int d;
    int i;
    char *s;

    mon[0]  = 0;
    mon[1]  = 31;
    mon[2]  = 29;
    mon[3]  = 31;
    mon[4]  = 30;
    mon[5]  = 31;
    mon[6]  = 30;
    mon[7]  = 31;
    mon[8]  = 31;
    mon[9]  = 30;
    mon[10] = 31;
    mon[11] = 30;
    mon[12] = 31;

    s = p;
    d = jan1(y);

    /* Determine leap year / 1752 exception */
    switch ((jan1(y + 1) + 7 - d) % 7) {
    case 1:            /* non-leap year */
        mon[2] = 28;
        break;
    case 2:            /* leap year */
        break;
    default:           /* 1752: September had 11 days removed */
        mon[9] = 19;
        break;
    }

    /* Advance d to start day of requested month */
    for (i = 1; i < m; i++)
        d = d + mon[i];
    d = d % 7;

    /* Position s at the correct starting column */
    s = s + 3 * d;

    for (i = 1; i <= mon[m]; i++) {
        /* 1752 Sep: skip days 3-13 (removed by Gregorian switch) */
        if (i == 3 && mon[m] == 19) {
            i = i + 11;
            mon[m] = mon[m] + 11;
        }

        /* Write day number into 3-char slot */
        if (i > 9)
            *s = i / 10 + '0';
        s++;
        *s = i % 10 + '0';
        s++;
        *s = ' ';
        s++;

        if (++d == 7) {
            d = 0;
            s = p + w;
            p = s;
        }
    }
}

/* ------------------------------------------------------------------ */
/* main                                                                 */
/* ------------------------------------------------------------------ */

int main(void) {
    int m;
    int y;
    int i;

    puts("cal - Unix V7 calendar");
    puts("----------------------");

    while (1) {
        print_str("Month (1-12, 0=quit): ");
        m = readint();

        if (m == 0 || m == -1)
            break;

        if (m < 1 || m > 12) {
            puts("Bad month (1-12)");
            continue;
        }

        print_str("Year  (1-9999):       ");
        y = readint();

        if (y < 1 || y > 9999) {
            puts("Bad year (1-9999)");
            continue;
        }

        /* Zero the grid buffer */
        memset(string, 0, 144);

        /* Print header: "   <MonthName> <Year>" */
        print_str("   ");
        print_str(month_name(m));
        putchar(' ');
        print_int(y);
        putchar(10);

        /* Print day-of-week header */
        puts(" S  M Tu  W Th  F  S");

        /* Fill and print the 6-row grid */
        cal(m, y, string, 24);
        for (i = 0; i < 6 * 24; i = i + 24)
            pstr(string + i, 24);

        putchar(10);
    }

    return 0;
}
