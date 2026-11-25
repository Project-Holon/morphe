/* From musl src/string/strrchr.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <string.h>

char *strrchr(const char *s, int c)
{
	return memrchr(s, c, strlen(s) + 1);
}
