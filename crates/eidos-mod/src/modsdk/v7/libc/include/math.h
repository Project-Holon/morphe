/* math.h of the mod C library: the functions that are one WebAssembly
 * instruction (src/math.c), and musl's fmod. */
#ifndef MOD_LIBC_MATH_H
#define MOD_LIBC_MATH_H

#define HUGE_VAL (__builtin_huge_val())
#define INFINITY (__builtin_inff())
#define NAN (__builtin_nanf(""))
#define isnan(x) __builtin_isnan(x)
#define isinf(x) __builtin_isinf(x)
#define isfinite(x) __builtin_isfinite(x)
#define signbit(x) __builtin_signbit(x)

double floor(double x);
double ceil(double x);
double trunc(double x);
double fabs(double x);
double sqrt(double x);
double fmod(double x, double y);
float floorf(float x);
float ceilf(float x);
float truncf(float x);
float fabsf(float x);
float sqrtf(float x);

#endif
