/* From musl src/ctype/islower.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>
#undef islower

int islower(int c)
{
	return (unsigned)c-'a' < 26;
}


