/* The calendar functions musl's time files share (in place of musl's
 * src/time/time_impl.h). */
#include <time.h>

int __month_to_secs(int month, int is_leap);
long long __year_to_secs(long long year, int *is_leap);
long long __tm_to_secs(const struct tm *tm);
int __secs_to_tm(long long t, struct tm *tm);
