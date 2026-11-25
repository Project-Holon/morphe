# The mod C library

A small C library for code mods (interface version 7). A mod is built for
`wasm32-freestanding` with no C library, and may import only the game's own
functions (`eidos`). This library gives a mod the common C functions without
any other import.

A mod gets the library only when it asks for it: kenon's "Add the C
library" copies it into the project (`include/` and `src/libc/`). The mod
then keeps that copy; a later version of the SDK never changes a mod that is
already built.

## What it has

| Header | Functions | From |
|---|---|---|
| string.h, strings.h | mem*, str*, stpcpy, strchrnul, strdup, strcasecmp | musl |
| ctype.h | is*, tolower, toupper (ASCII) | musl |
| stdlib.h | malloc, calloc, realloc, free | TLSF, in `src/malloc.c` |
| stdlib.h | strtol, strtoul, strtoll, strtoull | OpenBSD libc |
| stdlib.h | atoi, atol, atoll, abs, labs, llabs, qsort, qsort_r, bsearch, rand | musl |
| stdio.h | printf, sprintf, snprintf, vsnprintf, vsprintf, vprintf | eyalroz/printf |
| stdio.h | puts | `src/stdio.c` |
| time.h | gmtime, gmtime_r, timegm | musl |
| time.h | time, clock, mktime, localtime | `src/time.c` |
| math.h | fmod | musl |
| math.h | floor, ceil, trunc, fabs, sqrt | `src/math.c` (one WebAssembly instruction each) |
| errno.h, assert.h | errno, assert, abort | `src/misc.c` |

The compiler leaves out what a mod does not call, so a mod pays only for the
functions it uses.

## Memory

malloc takes its memory from one fixed block in the module's data:
`MOD_LIBC_HEAP_BYTES` at the top of `src/malloc.c`, **1 MiB** by default
(TLSF keeps about 3 KB of it for itself). The block is part of the module
only when the mod calls malloc. The module must never grow its memory:
eidos keeps its own heap above the module's data. Raise the constant when a
mod needs more, and keep the manifest's `memory` line at least as large as
the module's whole memory.

## Behaviour to know

- printf, vprintf and puts write to the mod log, one line per line break.
- %f of a number of 1e9 or more is printed as %e (printf's default
  `MAX_INTEGRAL_DIGITS_FOR_DECIMAL` of 9). %p is padded to the width of a
  pointer (`0x00000010`).
- time() and clock() ask `mod_libc_time_utc()` (seconds since 1970, UTC)
  and `mod_libc_ticks_ms()`, which the mod defines when it has a clock call
  of the game; without them both give 0. There are no time zones: mktime is
  timegm and localtime is gmtime.
- abort and a failed assert stop the mod with a trap; assert first writes
  its place to the mod log. Define NDEBUG to turn assert off.

## Where the code comes from

`tools/libc-vendor.py` in morphe copies the files from these checkouts and
makes the edits it lists (aliases, locales, musl's internal headers, one
warning switched off per file); each copied file says where it came from.
The licenses are in `licenses/`.

| Project | Version | License |
|---|---|---|
| musl | git 7813e91 (2026-10-14) | MIT, `licenses/musl-COPYRIGHT` |
| eyalroz/printf | git 22318ed (2026-06-27) | MIT, `licenses/printf-LICENSE` |
| mattconte/tlsf | git deff9ab (3.1) | BSD 3-clause, `licenses/tlsf-LICENSE` |
| OpenBSD libc | strtol.c 1.12, strtoul.c 1.11, strtoll.c 1.10, strtoull.c 1.9 | BSD 3-clause, `licenses/openbsd-LICENSE` |

The headers and the files in `src/` that name no project above (malloc.c,
stdio.c, time.c, math.c, misc.c, time_impl.h) are the library's own: the
glue between the copied code and a mod.
