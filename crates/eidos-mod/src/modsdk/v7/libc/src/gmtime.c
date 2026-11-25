/* From musl src/time/gmtime.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include "time_impl.h"
#include <errno.h>

struct tm *gmtime(const time_t *t)
{
	static struct tm tm;
	return gmtime_r(t, &tm);
}
