/* From musl src/ctype/tolower.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>

int tolower(int c)
{
	if (isupper(c)) return c | 32;
	return c;
}


