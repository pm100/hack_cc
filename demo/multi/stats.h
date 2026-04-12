/* stats.h — Simple array statistics library (header / forward declarations) */
#ifndef STATS_H
#define STATS_H

/* Sum of all elements. */
int stats_sum(int *arr, int n);

/* Average (integer division). */
int stats_avg(int *arr, int n);

/* Minimum value. */
int stats_min(int *arr, int n);

/* Maximum value. */
int stats_max(int *arr, int n);

/* Count values equal to target. */
int stats_count(int *arr, int n, int target);

#endif
