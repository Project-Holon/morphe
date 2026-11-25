/* From musl src/string/strnlen.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <string.h>

size_t strnlen(const char *s, size_t n)
{
	const char *p = memchr(s, 0, n);
	return p ? p-s : n;
}
