/* From musl src/prng/rand.c; edited by tools/libc-vendor.py (see libc/README.md). */
#pragma clang diagnostic ignored "-Wsign-compare"
#include <stdlib.h>
#include <stdint.h>

static uint64_t seed;

void srand(unsigned s)
{
	seed = s-1;
}

int rand(void)
{
	seed = 6364136223846793005ULL*seed + 1;
	return seed>>33;
}
