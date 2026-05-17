/*
 * Optional profiling hooks emitted into LLVM IR when using kernlc --instrument-llvm.
 * Safe no-op implementations; override in your own build for real telemetry.
 */
#include <stdint.h>
#include <stdio.h>

void __kernl_profile_enter(const char *name) {
    (void)name;
}

void __kernl_profile_exit(const char *name) {
    (void)name;
}

void __kernl_profile_report(void) {
    /* Hook: dump aggregated timings here if you replace enter/exit */
}
