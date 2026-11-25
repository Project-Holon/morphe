/* stdlib.h of the mod C library: memory (TLSF, src/malloc.c), numbers from
 * text (OpenBSD's strto*, musl's ato*), sorting and searching (musl). */
#ifndef MOD_LIBC_STDLIB_H
#define MOD_LIBC_STDLIB_H

#include <stddef.h>

#define EXIT_SUCCESS 0
#define EXIT_FAILURE 1
#define RAND_MAX 0x7fffffff

void *malloc(size_t size);
void *calloc(size_t count, size_t size);
void *realloc(void *block, size_t size);
void free(void *block);

long strtol(const char *restrict s, char **restrict end, int base);
unsigned long strtoul(const char *restrict s, char **restrict end, int base);
long long strtoll(const char *restrict s, char **restrict end, int base);
unsigned long long strtoull(const char *restrict s, char **restrict end, int base);
int atoi(const char *s);
long atol(const char *s);
long long atoll(const char *s);

int abs(int v);
long labs(long v);
long long llabs(long long v);

void qsort(void *base, size_t count, size_t size, int (*compare)(const void *, const void *));
void qsort_r(void *base, size_t count, size_t size,
             int (*compare)(const void *, const void *, void *), void *arg);
void *bsearch(const void *key, const void *base, size_t count, size_t size,
              int (*compare)(const void *, const void *));

int rand(void);
void srand(unsigned seed);

/* A mod cannot end the game: abort stops the mod with a trap. */
_Noreturn void abort(void);

#endif
