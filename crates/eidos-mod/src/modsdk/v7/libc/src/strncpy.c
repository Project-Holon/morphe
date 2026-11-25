/* From musl src/string/strncpy.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <string.h>

char *strncpy(char *restrict d, const char *restrict s, size_t n)
{
	stpncpy(d, s, n);
	return d;
}
