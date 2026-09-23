// Stub for mars/comm/assert/__assert.c.
//
// The real implementation logs through `xlogger_*`, which drags in the whole
// appender. Nothing in this harness is expected to trip an ASSERT, so the
// helpers just report and abort; that keeps the C++ side of the test to the
// crypt + buffer sources only.
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>

extern "C" {

void __ASSERTV2(const char*, int, const char*, const char*, const char*, va_list);

void __ASSERT(const char* file, int line, const char* func, const char* expr) {
    fprintf(stderr, "ASSERT failed: %s (%s:%d %s)\n", expr, file, line, func);
    abort();
}

void __ASSERT2(const char* file,
               int line,
               const char* func,
               const char* expr,
               const char* fmt,
               ...) {
    va_list ap;
    va_start(ap, fmt);
    __ASSERTV2(file, line, func, expr, fmt, ap);
    va_end(ap);
}

void __ASSERTV2(const char* file,
                int line,
                const char* func,
                const char* expr,
                const char* fmt,
                va_list ap) {
    fprintf(stderr, "ASSERT failed: %s (%s:%d %s): ", expr, file, line, func);
    vfprintf(stderr, fmt, ap);
    fputc('\n', stderr);
    abort();
}

void ENABLE_ASSERT() {
}
void DISABLE_ASSERT() {
}
int IS_ASSERT_ENABLE() {
    return 1;
}

}  // extern "C"
