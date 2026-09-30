/* A stand-in for a Souther library: an arena of its own that is a count, souther_scope_open and
   souther_scope_close over it, the ABI marker, a behavior that answers, and a behavior a host
   implements, which it calls back through the capability it was handed. */
#include "fake.h"
#include <stdio.h>
#include <stdlib.h>

#ifndef ABI
#define ABI 9
#endif

/* A library with a thread-local variable is one macOS does not unload, as the real one is; a test of
   unloading one that failed to load asks a library without one. */
#ifdef NO_TLS
#define PER_THREAD
#else
#define PER_THREAD __thread
#endif
static PER_THREAD int64_t arena;

/* The scopes open on a thread, innermost last, each a token and where the arena stood. */
static PER_THREAD int64_t open_tokens[64];
static PER_THREAD int64_t open_at[64];
static PER_THREAD int depth;
static int64_t tokens;

int64_t souther_scope_open(void) {
    int64_t token = __atomic_add_fetch(&tokens, 1, __ATOMIC_RELAXED);
    open_tokens[depth] = token;
    open_at[depth] = arena;
    depth++;
    return token;
}

uint8_t souther_scope_close(int64_t scope) {
    if (depth == 0 || open_tokens[depth - 1] != scope) {
        return 0;
    }
    depth--;
    arena = open_at[depth];
    return 1;
}

#define CAT(a, b) CAT_(a, b)
#define CAT_(a, b) a##b
int CAT(souther_runtime_abi_, ABI)(void) { return ABI; }
#ifndef NO_GENERATION_QUERY
uint32_t souther_abi_generation(void) { return ABI; }
#endif

#ifndef NO_DOUBLE
souther_status fake_double(int64_t x, int64_t *out) {
    arena++;
    *out = x * 2;
    return 0;
}
#endif

typedef souther_status (*invoker)(const souther_capability *, int64_t, int64_t *);

/* What stands in the rooms a host lays out, as the library's generated code alone reads them: the
   host sees as many slots as each takes and no field. */
struct capability_view {
    void *invoke;
    const void *environment;
};
struct hosted_view {
    void *implementation;
    void *userdata;
};
#define CAPABILITY(at) ((struct capability_view *)(at))
#define CONST_CAPABILITY(at) ((const struct capability_view *)(at))
#define HOSTED(at) ((struct hosted_view *)(at))

/* A capability is called through what it holds to call itself with, as a library's is. */
static souther_status invoke_hosted(const souther_capability *capability, int64_t x, int64_t *out) {
    const struct hosted_view *hosted = CONST_CAPABILITY(capability)->environment;
    return ((fake_implementation)hosted->implementation)(hosted->userdata, x, out);
}

/* A behavior bound to what it requires asks the first. */
static souther_status invoke_bound(const souther_capability *capability, int64_t x, int64_t *out) {
    const souther_capability *const *requirements = CONST_CAPABILITY(capability)->environment;
    return fake_call(requirements[0], x, out);
}

void fake_implement(souther_capability *into, souther_hosted *hosted, fake_implementation implementation,
                    void *userdata) {
    HOSTED(hosted)->implementation = (void *)implementation;
    HOSTED(hosted)->userdata = userdata;
    CAPABILITY(into)->invoke = (void *)invoke_hosted;
    CAPABILITY(into)->environment = hosted;
}

souther_status fake_call(const souther_capability *capability, int64_t x, int64_t *out) {
    return ((invoker)CONST_CAPABILITY(capability)->invoke)(capability, x, out);
}

void fake_bind(souther_capability *into, const souther_capability *const *requirements) {
    CAPABILITY(into)->invoke = (void *)invoke_bound;
    CAPABILITY(into)->environment = requirements;
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
