/* What a library's souther.ffi.h declares of a behavior the host implements, and of one it calls:
   declarations only, so that a cgo preamble can include it. */
#include <stdint.h>

typedef uint32_t souther_status;
typedef struct souther_capability {
    void *invoke;
    const void *environment;
} souther_capability;
typedef struct souther_hosted {
    void *implementation;
    void *userdata;
} souther_hosted;
typedef souther_status (*fake_implementation)(void *, int64_t, int64_t *);

souther_status fake_call(const souther_capability *, int64_t, int64_t *);
souther_status fake_double(int64_t, int64_t *);
void fake_implement(souther_capability *, souther_hosted *, fake_implementation, void *);
