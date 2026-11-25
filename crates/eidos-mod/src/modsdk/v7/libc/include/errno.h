/* errno.h of the mod C library: the strto* functions set ERANGE, the
 * calendar EOVERFLOW. */
#ifndef MOD_LIBC_ERRNO_H
#define MOD_LIBC_ERRNO_H

extern int errno;

#define ENOMEM 12
#define EINVAL 22
#define EDOM 33
#define ERANGE 34
#define EOVERFLOW 75

#endif
