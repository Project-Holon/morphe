/*
 * The clock and the calls that would need time zones. The calendar itself
 * (gmtime_r, gmtime, timegm) is musl's.
 *
 * The library has no clock: time() and clock() ask mod_libc_time_utc and
 * mod_libc_ticks_ms, which a mod with a clock call of the game defines;
 * these weak ones return 0. A mod has no time zone, so local time is UTC:
 * mktime is timegm and localtime is gmtime.
 */

#include <time.h>

__attribute__((weak)) long long mod_libc_time_utc(void)
{
    return 0;
}

__attribute__((weak)) long long mod_libc_ticks_ms(void)
{
    return 0;
}

time_t time(time_t *out)
{
    time_t t = (time_t)mod_libc_time_utc();
    if (out) {
        *out = t;
    }
    return t;
}

clock_t clock(void)
{
    return (clock_t)mod_libc_ticks_ms();
}

double difftime(time_t end, time_t start)
{
    return (double)(end - start);
}

time_t mktime(struct tm *tm)
{
    return timegm(tm);
}

struct tm *localtime_r(const time_t *t, struct tm *out)
{
    return gmtime_r(t, out);
}

struct tm *localtime(const time_t *t)
{
    return gmtime(t);
}
