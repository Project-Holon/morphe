/* From musl src/ctype/iscntrl.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <ctype.h>

int iscntrl(int c)
{
	return (unsigned)c < 0x20 || c == 0x7f;
}


