/* time.h of the mod C library. The calendar is musl's (gmtime, gmtime_r,
 * timegm); every time is UTC. The library has no clock of its own: time()
 * and clock() ask mod_libc_time_utc and mod_libc_ticks_ms, which the mod
 * may define (both give 0 when it does not). src/time.c. */
#ifndef MOD_LIBC_TIME_H
#define MOD_LIBC_TIME_H

#include <stddef.h>

typedef long long time_t;
typedef long clock_t;
#define CLOCKS_PER_SEC 1000L

struct tm {
    int tm_sec;
    int tm_min;
    int tm_hour;
    int tm_mday;
    int tm_mon;
    int tm_year;
    int tm_wday;
    int tm_yday;
    int tm_isdst;
};

time_t time(time_t *out);
clock_t clock(void);
double difftime(time_t end, time_t start);
time_t timegm(struct tm *tm);
time_t mktime(struct tm *tm);
struct tm *gmtime(const time_t *t);
struct tm *gmtime_r(const time_t *restrict t, struct tm *restrict out);
struct tm *localtime(const time_t *t);
struct tm *localtime_r(const time_t *restrict t, struct tm *restrict out);

/* Defined by the mod to give the library a clock: seconds since 1970 (UTC)
 * and milliseconds from any fixed start. */
long long mod_libc_time_utc(void);
long long mod_libc_ticks_ms(void);

#endif
