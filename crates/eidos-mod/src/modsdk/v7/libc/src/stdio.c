/*
 * Where printf's text goes. printf.c formats; it hands each character of
 * printf and vprintf to putchar_. A mod has no files: the characters make
 * up lines of the mod log (eidos_log), one per line break or full buffer.
 */

#include <stdint.h>
#include <string.h>

#include "eidos_wasm.h"

static char line[512];
static uint32_t used;

static void flush(void)
{
    eidos_log(line, used);
    used = 0;
}

void putchar_(char c)
{
    if (c == '\n') {
        flush();
        return;
    }
    line[used++] = c;
    if (used == sizeof line) {
        flush();
    }
}

int puts(const char *text)
{
    if (used) {
        flush();
    }
    eidos_log(text, (uint32_t)strlen(text));
    return 0;
}
