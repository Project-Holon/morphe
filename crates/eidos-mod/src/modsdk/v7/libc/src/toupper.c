/* From musl src/ctype/toupper.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>

int toupper(int c)
{
	if (islower(c)) return c & 0x5f;
	return c;
}


