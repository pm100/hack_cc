/* stats.c — Simple array statistics library implementation */
#include <hack.h>
#include "stats.h"

int stats_sum(int *arr, int n) {
    int i;
    int s;
    s = 0;
    i = 0;
    while (i < n) {
        s = s + arr[i];
        i = i + 1;
    }
    return s;
}

int stats_avg(int *arr, int n) {
    if (n == 0) return 0;
    return stats_sum(arr, n) / n;
}

int stats_min(int *arr, int n) {
    int i;
    int m;
    if (n == 0) return 0;
    m = arr[0];
    i = 1;
    while (i < n) {
        if (arr[i] < m) m = arr[i];
        i = i + 1;
    }
    return m;
}

int stats_max(int *arr, int n) {
    int i;
    int m;
    if (n == 0) return 0;
    m = arr[0];
    i = 1;
    while (i < n) {
        if (arr[i] > m) m = arr[i];
        i = i + 1;
    }
    return m;
}

int stats_count(int *arr, int n, int target) {
    int i;
    int c;
    c = 0;
    i = 0;
    while (i < n) {
        if (arr[i] == target) c = c + 1;
        i = i + 1;
    }
    return c;
}
