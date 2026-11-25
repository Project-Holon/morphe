/* From musl src/string/strcasecmp.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <strings.h>
#include <ctype.h>

int strcasecmp(const char *_l, const char *_r)
{
	const unsigned char *l=(void *)_l, *r=(void *)_r;
	for (; *l && *r && (*l == *r || tolower(*l) == tolower(*r)); l++, r++);
	return tolower(*l) - tolower(*r);
}


