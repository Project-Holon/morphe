/* From musl src/ctype/isprint.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>
#undef isprint

int isprint(int c)
{
	return (unsigned)c-0x20 < 0x5f;
}


