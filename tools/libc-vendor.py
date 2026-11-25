"""Copies the mod C library's code from its upstream projects into
crates/eidos-mod/src/modsdk/v7/libc/src, with the small edits listed below,
and the license of each project into libc/licenses.

    python -I tools/libc-vendor.py <musl> <printf> <tlsf> <openbsd>

<musl> is a musl checkout (git.musl-libc.org/musl), <printf> an
eyalroz/printf checkout, <tlsf> a mattconte/tlsf checkout, and <openbsd> a
folder with OpenBSD's lib/libc/stdlib/strto{l,ul,ll,ull}.c. Each copied file
starts with a line naming where it came from. The edits only remove what a
mod has no use for or cannot build (locales, aliases, musl's internal
headers); they change no code path:

- every `weak_alias(__x, x)` line goes, and `__x` is renamed `x` in all the
  copied files (WebAssembly modules cannot rely on aliases);
- the locale variants (functions ending in `_l` that take a `locale_t`) and
  their aliases go;
- musl's `atomic.h` gives `a_ctz_l` (count trailing zeros): the compiler's
  `__builtin_ctzl` takes its place;
- `<endian.h>` goes; `__BYTE_ORDER == __LITTLE_ENDIAN` becomes the
  compiler's `__BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__`;
- musl's `time_impl.h` is replaced by the library's own (declarations);
  the struct tm time zone fields musl sets (`__tm_gmtoff`, `__tm_zone`) go,
  as every time here is UTC;
- OpenBSD's `DEF_STRONG` and `__weak_alias` lines and `<sys/types.h>` go;
- printf.h turns on PRINTF_ALIAS_STANDARD_FUNCTION_NAMES_HARD, so the file
  defines printf, snprintf and the rest under their standard names;
- each file gets a `#pragma clang diagnostic ignored "-Wsign-compare"`
  line: the mod build's -Wextra warns on comparisons the upstream code
  makes on purpose.
"""
import os
import re
import shutil
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
LIBC = os.path.join(ROOT, "crates", "eidos-mod", "src", "modsdk", "v7", "libc")

MUSL = [
    "string/memcpy.c", "string/memmove.c", "string/memset.c", "string/memcmp.c",
    "string/memchr.c", "string/memrchr.c", "string/strlen.c", "string/strnlen.c",
    "string/strcmp.c", "string/strncmp.c", "string/strcasecmp.c", "string/strncasecmp.c",
    "string/strcpy.c", "string/stpcpy.c", "string/strncpy.c", "string/stpncpy.c",
    "string/strcat.c", "string/strncat.c", "string/strchr.c", "string/strchrnul.c",
    "string/strrchr.c", "string/strstr.c", "string/strspn.c", "string/strcspn.c",
    "string/strdup.c", "string/strndup.c",
    "ctype/isalnum.c", "ctype/isalpha.c", "ctype/isblank.c", "ctype/iscntrl.c",
    "ctype/isdigit.c", "ctype/isgraph.c", "ctype/islower.c", "ctype/isprint.c",
    "ctype/ispunct.c", "ctype/isspace.c", "ctype/isupper.c", "ctype/isxdigit.c",
    "ctype/tolower.c", "ctype/toupper.c",
    "stdlib/qsort.c", "stdlib/qsort_nr.c", "stdlib/bsearch.c", "stdlib/atoi.c",
    "stdlib/atol.c", "stdlib/atoll.c", "stdlib/abs.c", "stdlib/labs.c", "stdlib/llabs.c",
    "prng/rand.c",
    "math/fmod.c",
    "time/__secs_to_tm.c", "time/__tm_to_secs.c", "time/__year_to_secs.c",
    "time/__month_to_secs.c", "time/gmtime_r.c", "time/gmtime.c", "time/timegm.c",
]
OPENBSD = ["strtol.c", "strtoul.c", "strtoll.c", "strtoull.c"]


def musl_edit(text):
    # aliases: drop the line, give the function its public name
    renames = re.findall(r"^weak_alias\((__\w+),\s*(\w+)\);\s*$", text, re.M)
    text = re.sub(r"^weak_alias\(.*\);\s*\n", "", text, flags=re.M)
    # locale variants: a top-level function whose name ends in _l
    text = re.sub(r"\n[^\n;{}]*\b__?\w+_l\([^)]*locale_t[^)]*\)\s*\{.*?\n\}\n", "\n", text, flags=re.S)
    text = text.replace('#include "atomic.h"', "#define a_ctz_l(x) __builtin_ctzl(x)")
    text = text.replace("#include <endian.h>\n", "")
    text = text.replace("__BYTE_ORDER == __LITTLE_ENDIAN", "__BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__")
    text = re.sub(r"^\s*tm->__tm_(gmtoff|zone) = .*;\n", "", text, flags=re.M)
    return text, renames


def openbsd_edit(text):
    text = re.sub(r"^(DEF_STRONG|__weak_alias)\(.*\);\s*\n", "", text, flags=re.M)
    return text.replace("#include <sys/types.h>\n", "")


def head(origin, text):
    # upstream code is kept as it is; the warnings the mod build turns on
    # (-Wall -Wextra) and it does not meet are named here, not fixed there
    return (
        f"/* From {origin}; edited by tools/libc-vendor.py (see libc/README.md). */\n"
        '#pragma clang diagnostic ignored "-Wsign-compare"\n'
        f"{text}"
    )


def main():
    musl, printf, tlsf, openbsd = sys.argv[1:5]
    src = os.path.join(LIBC, "src")
    lic = os.path.join(LIBC, "licenses")
    os.makedirs(src, exist_ok=True)
    os.makedirs(lic, exist_ok=True)
    out = {}
    renames = []
    for rel in MUSL:
        text = open(os.path.join(musl, "src", rel), encoding="utf-8").read()
        text, r = musl_edit(text)
        renames += r
        name = os.path.basename(rel)
        out[name] = head(f"musl src/{rel}", text)
    for name in OPENBSD:
        text = open(os.path.join(openbsd, name), encoding="utf-8").read()
        out[name] = head(f"OpenBSD lib/libc/stdlib/{name}", openbsd_edit(text))
    out["printf.c"] = head("eyalroz/printf src/printf/printf.c",
                           open(os.path.join(printf, "src", "printf", "printf.c"), encoding="utf-8").read())
    out["tlsf.c"] = head("mattconte/tlsf tlsf.c", open(os.path.join(tlsf, "tlsf.c"), encoding="utf-8").read())
    out["tlsf.h"] = head("mattconte/tlsf tlsf.h", open(os.path.join(tlsf, "tlsf.h"), encoding="utf-8").read())
    for old, new in renames:
        for name in out:
            out[name] = re.sub(rf"\b{old}\b", new, out[name])
    for name, text in out.items():
        open(os.path.join(src, name), "w", encoding="utf-8", newline="\n").write(text)
    # printf.h goes with the headers, under printf/ as printf.c includes it
    h = open(os.path.join(printf, "src", "printf", "printf.h"), encoding="utf-8").read()
    h = h.replace("#define PRINTF_H_\n", "#define PRINTF_H_\n#define PRINTF_ALIAS_STANDARD_FUNCTION_NAMES_HARD 1\n", 1)
    os.makedirs(os.path.join(LIBC, "include", "printf"), exist_ok=True)
    open(os.path.join(LIBC, "include", "printf", "printf.h"), "w", encoding="utf-8", newline="\n").write(
        head("eyalroz/printf src/printf/printf.h", h))
    # licenses: musl's COPYRIGHT, printf's LICENSE, and the BSD notices the
    # TLSF and OpenBSD files carry in their heads
    shutil.copy(os.path.join(musl, "COPYRIGHT"), os.path.join(lic, "musl-COPYRIGHT"))
    shutil.copy(os.path.join(printf, "LICENSE"), os.path.join(lic, "printf-LICENSE"))
    tl = open(os.path.join(tlsf, "tlsf.h"), encoding="utf-8").read()
    notice = re.search(r"/\*+\s*\n(.*?Copyright.*?)\*/", tl, re.S)
    open(os.path.join(lic, "tlsf-LICENSE"), "w", encoding="utf-8", newline="\n").write(
        re.sub(r"^\*\* ?", "", notice.group(1), flags=re.M) if notice else tl)
    ob = open(os.path.join(openbsd, "strtol.c"), encoding="utf-8").read()
    notice = re.search(r"/\*\n( \* Copyright.*?)\*/", ob, re.S)
    open(os.path.join(lic, "openbsd-LICENSE"), "w", encoding="utf-8", newline="\n").write(
        re.sub(r"^ \* ?", "", notice.group(1), flags=re.M))
    print(f"{len(out)} files, renamed {', '.join(o for o, _ in renames)}")


if __name__ == "__main__":
    main()
