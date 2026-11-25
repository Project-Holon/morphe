/* stdio.h of the mod C library. eyalroz/printf (src/printf.c) gives
 * printf, sprintf, snprintf, vsnprintf, vsprintf and vprintf; a mod has no
 * files, so printf and puts write lines to the mod log (src/stdio.c). */
#ifndef MOD_LIBC_STDIO_H
#define MOD_LIBC_STDIO_H

#include <printf/printf.h>

#define EOF (-1)

int puts(const char *text);

#endif
