/*
 * malloc, calloc, realloc and free over TLSF (tlsf.c).
 *
 * The memory is one fixed block in the module's data, MOD_LIBC_HEAP_BYTES.
 * eidos keeps its own heap in the module's memory above the data, so the
 * module must not grow its memory or use memory past its data: raise the
 * constant for more, and keep the manifest's `memory` line at least as
 * large as the module needs. TLSF takes about 3 KB of the block for itself.
 */

#include <stddef.h>
#include <string.h>

#include "tlsf.h"

/* The one place the heap size is set. */
#define MOD_LIBC_HEAP_BYTES (1024u * 1024u)

static unsigned char heap[MOD_LIBC_HEAP_BYTES] __attribute__((aligned(8)));
static tlsf_t pool;

static tlsf_t get_pool(void)
{
    if (!pool) {
        pool = tlsf_create_with_pool(heap, sizeof heap);
    }
    return pool;
}

void *malloc(size_t size)
{
    return tlsf_malloc(get_pool(), size);
}

void free(void *block)
{
    if (block) {
        tlsf_free(get_pool(), block);
    }
}

void *realloc(void *block, size_t size)
{
    return tlsf_realloc(get_pool(), block, size);
}

void *calloc(size_t count, size_t size)
{
    void *p;
    if (size && count > (size_t)-1 / size) {
        return NULL;
    }
    p = malloc(count * size);
    if (p) {
        memset(p, 0, count * size);
    }
    return p;
}

/* The names of the SDK's first allocator (alloc.c), for mods written
 * against it. */
void *mod_alloc(size_t size)
{
    return malloc(size);
}

void mod_free(void *block)
{
    free(block);
}
