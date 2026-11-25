/* From musl src/ctype/ispunct.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>

int ispunct(int c)
{
	return isgraph(c) && !isalnum(c);
}


