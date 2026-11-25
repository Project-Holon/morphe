/* From musl src/string/strcpy.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <string.h>

char *strcpy(char *restrict dest, const char *restrict src)
{
	stpcpy(dest, src);
	return dest;
}
