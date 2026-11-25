/*
 * floor, ceil, trunc, fabs and sqrt are one WebAssembly instruction each
 * (f64.floor and so on): the compiler's builtins give them. fmod is musl's.
 */

#include <math.h>

double floor(double x) { return __builtin_floor(x); }
double ceil(double x) { return __builtin_ceil(x); }
double trunc(double x) { return __builtin_trunc(x); }
double fabs(double x) { return __builtin_fabs(x); }
double sqrt(double x) { return __builtin_sqrt(x); }
float floorf(float x) { return __builtin_floorf(x); }
float ceilf(float x) { return __builtin_ceilf(x); }
float truncf(float x) { return __builtin_truncf(x); }
float fabsf(float x) { return __builtin_fabsf(x); }
float sqrtf(float x) { return __builtin_sqrtf(x); }
