/* From musl src/stdlib/llabs.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <stdlib.h>

long long llabs(long long a)
{
	return a>0 ? a : -a;
}
