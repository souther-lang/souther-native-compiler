/* A stand-in for a Souther library: an arena of its own that is a count, souther_mark and
   souther_reset over it, the ABI marker, a behavior that answers, and a behavior a host
   implements, which it calls back through the capability it was handed. */
#include "fake.h"
#include <stdio.h>
#include <stdlib.h>

#ifndef ABI
#define ABI 8
#endif

/* A library with a thread-local variable is one macOS does not unload, as the real one is; a test of
   unloading one that failed to load asks a library without one. */
#ifdef NO_TLS
static int64_t arena;
#else
static __thread int64_t arena;
#endif

int64_t souther_mark(void) { return arena; }
void souther_reset(int64_t mark) { arena = mark; }

#define CAT(a, b) CAT_(a, b)
#define CAT_(a, b) a##b
int CAT(souther_runtime_abi_, ABI)(void) { return ABI; }

#ifndef NO_DOUBLE
souther_status fake_double(int64_t x, int64_t *out) {
    arena++;
    *out = x * 2;
    return 0;
}
#endif

typedef souther_status (*invoker)(const souther_capability *, int64_t, int64_t *);

/* A capability is called through what it holds to call itself with, as a library's is. */
static souther_status invoke_hosted(const souther_capability *capability, int64_t x, int64_t *out) {
    const souther_hosted *hosted = capability->environment;
    return ((fake_implementation)hosted->implementation)(hosted->userdata, x, out);
}

/* A behavior bound to what it requires asks the first. */
static souther_status invoke_bound(const souther_capability *capability, int64_t x, int64_t *out) {
    const souther_capability *const *requirements = capability->environment;
    return fake_call(requirements[0], x, out);
}

void fake_implement(souther_capability *into, souther_hosted *hosted, fake_implementation implementation,
                    void *userdata) {
    hosted->implementation = (void *)implementation;
    hosted->userdata = userdata;
    into->invoke = (void *)invoke_hosted;
    into->environment = hosted;
}

souther_status fake_call(const souther_capability *capability, int64_t x, int64_t *out) {
    return ((invoker)capability->invoke)(capability, x, out);
}

void fake_bind(souther_capability *into, const souther_capability *const *requirements) {
    into->invoke = (void *)invoke_bound;
    into->environment = requirements;
}

/* A behavior bound to what it requires: it asks the first requirement. */
souther_status fake_run(const souther_capability *const *requirements, int64_t x, int64_t *out) {
    return fake_call(requirements[0], x, out);
}

/* Says that the file was unloaded, for a test that asks whether a load that failed unloaded it. */
__attribute__((destructor)) static void unloaded(void) {
    const char *where = getenv("SOUTHER_FAKE_UNLOADED");
    if (where != NULL) {
        FILE *file = fopen(where, "a");
        if (file != NULL) {
            fputs("x", file);
            fclose(file);
        }
    }
}
