/* From musl src/ctype/isblank.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>

int isblank(int c)
{
	return (c == ' ' || c == '\t');
}


