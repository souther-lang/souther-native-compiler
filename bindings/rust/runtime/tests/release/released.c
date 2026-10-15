/* A stand-in for a library of the generation the runtime calls: it answers the generation query,
   and its souther_release writes that it was called to the file SOUTHER_RELEASED names. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

uint32_t souther_abi_generation(void) { return ABI; }

void souther_release(void) {
    const char *path = getenv("SOUTHER_RELEASED");
    FILE *file = path ? fopen(path, "a") : NULL;
    if (file) {
        fputs("released\n", file);
        fclose(file);
    }
}
