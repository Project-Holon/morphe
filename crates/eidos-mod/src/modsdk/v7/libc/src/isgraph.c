/* From musl src/ctype/isgraph.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>
#undef isgraph

int isgraph(int c)
{
	return (unsigned)c-0x21 < 0x5e;
}


