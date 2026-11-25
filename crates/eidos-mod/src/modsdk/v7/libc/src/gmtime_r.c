/* From musl src/time/gmtime_r.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include "time_impl.h"
#include <errno.h>

struct tm *gmtime_r(const time_t *restrict t, struct tm *restrict tm)
{
	if (__secs_to_tm(*t, tm) < 0) {
		errno = EOVERFLOW;
		return 0;
	}
	tm->tm_isdst = 0;
	return tm;
}

