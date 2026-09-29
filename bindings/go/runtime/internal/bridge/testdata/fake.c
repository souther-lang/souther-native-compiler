/* A stand-in for a Souther library: an arena of its own that is a count, souther_mark and
   souther_reset over it, the ABI marker, a behavior that answers, and a behavior a host
   implements, which it calls back through the capability it was handed. */
#include "fake.h"

#ifndef ABI
#define ABI 8
#endif

static __thread int64_t arena;

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

void fake_implement(souther_capability *into, souther_hosted *hosted, fake_implementation implementation,
                    void *userdata) {
    hosted->implementation = (void *)implementation;
    hosted->userdata = userdata;
    into->invoke = 0;
    into->environment = hosted;
}

souther_status fake_call(const souther_capability *capability, int64_t x, int64_t *out) {
    const souther_hosted *hosted = capability->environment;
    return ((fake_implementation)hosted->implementation)(hosted->userdata, x, out);
}
