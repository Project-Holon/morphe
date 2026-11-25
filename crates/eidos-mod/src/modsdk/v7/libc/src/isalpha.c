/* From musl src/ctype/isalpha.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>
#undef isalpha

int isalpha(int c)
{
	return ((unsigned)c|32)-'a' < 26;
}


