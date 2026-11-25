/* From musl src/ctype/isdigit.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>
#undef isdigit

int isdigit(int c)
{
	return (unsigned)c-'0' < 10;
}


