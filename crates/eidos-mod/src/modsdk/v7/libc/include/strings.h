/* strings.h of the mod C library: musl's strcasecmp and strncasecmp, for
 * ASCII. */
#ifndef MOD_LIBC_STRINGS_H
#define MOD_LIBC_STRINGS_H

#include <stddef.h>

int strcasecmp(const char *a, const char *b);
int strncasecmp(const char *a, const char *b, size_t count);

#endif
