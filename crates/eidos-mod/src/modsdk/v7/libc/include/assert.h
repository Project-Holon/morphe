/* assert for the mod C library. A failed assert writes its place to the mod
 * log and stops the mod with a trap. Define NDEBUG to turn the checks off. */
#undef assert
#ifdef NDEBUG
#define assert(cond) ((void)0)
#else
void mod_libc_assert_failed(const char *cond, const char *file, int line);
#define assert(cond) ((cond) ? (void)0 : mod_libc_assert_failed(#cond, __FILE__, __LINE__))
#endif

#ifndef static_assert
#define static_assert _Static_assert
#endif
