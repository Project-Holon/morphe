/* From musl src/stdlib/labs.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <stdlib.h>

long labs(long a)
{
	return a>0 ? a : -a;
}
