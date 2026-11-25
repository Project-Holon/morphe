/* From musl src/ctype/isxdigit.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>

int isxdigit(int c)
{
	return isdigit(c) || ((unsigned)c|32)-'a' < 6;
}


