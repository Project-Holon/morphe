/*
 * errno, abort and assert's report. A mod cannot end the game: abort and a
 * failed assert stop the mod with a trap.
 */

#include <stdio.h>
#include <stdlib.h>

#include "eidos_wasm.h"

int errno;

_Noreturn void abort(void)
{
    __builtin_trap();
}

void mod_libc_assert_failed(const char *cond, const char *file, int line)
{
    char text[256];
    int n = snprintf(text, sizeof text, "assert failed: %s (%s:%d)", cond, file, line);
    eidos_log(text, (uint32_t)(n < (int)sizeof text ? n : (int)sizeof text - 1));
    __builtin_trap();
}
