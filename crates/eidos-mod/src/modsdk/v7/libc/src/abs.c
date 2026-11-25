/* From musl src/stdlib/abs.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <stdlib.h>

int abs(int a)
{
	return a>0 ? a : -a;
}
