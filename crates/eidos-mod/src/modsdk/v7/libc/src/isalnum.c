/* From musl src/ctype/isalnum.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>

int isalnum(int c)
{
	return isalpha(c) || isdigit(c);
}


