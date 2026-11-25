/* From musl src/ctype/isspace.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>
#undef isspace

int isspace(int c)
{
	return c == ' ' || (unsigned)c-'\t' < 5;
}


