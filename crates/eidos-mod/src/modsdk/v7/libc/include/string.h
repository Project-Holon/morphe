/* string.h of the mod C library: musl's string functions (libc/src). */
#ifndef MOD_LIBC_STRING_H
#define MOD_LIBC_STRING_H

#include <stddef.h>

void *memcpy(void *restrict dst, const void *restrict src, size_t count);
void *memmove(void *dst, const void *src, size_t count);
void *memset(void *dst, int value, size_t count);
int memcmp(const void *a, const void *b, size_t count);
void *memchr(const void *s, int c, size_t count);
void *memrchr(const void *s, int c, size_t count);

size_t strlen(const char *s);
size_t strnlen(const char *s, size_t max);
int strcmp(const char *a, const char *b);
int strncmp(const char *a, const char *b, size_t count);
char *strcpy(char *restrict dst, const char *restrict src);
char *stpcpy(char *restrict dst, const char *restrict src);
char *strncpy(char *restrict dst, const char *restrict src, size_t count);
char *stpncpy(char *restrict dst, const char *restrict src, size_t count);
char *strcat(char *restrict dst, const char *restrict src);
char *strncat(char *restrict dst, const char *restrict src, size_t count);
char *strchr(const char *s, int c);
char *strchrnul(const char *s, int c);
char *strrchr(const char *s, int c);
char *strstr(const char *haystack, const char *needle);
size_t strspn(const char *s, const char *accept);
size_t strcspn(const char *s, const char *reject);
char *strdup(const char *s);
char *strndup(const char *s, size_t max);

#include <strings.h>

#endif
