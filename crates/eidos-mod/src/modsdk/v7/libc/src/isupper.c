/* From musl src/ctype/isupper.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>
#undef isupper

int isupper(int c)
{
	return (unsigned)c-'A' < 26;
}


