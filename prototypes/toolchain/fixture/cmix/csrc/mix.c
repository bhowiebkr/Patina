/* The C code of the toolchain prototype. It uses the C library, so the build needs its headers
   and the linker needs its functions. */
#include <stdio.h>
#include <string.h>

int mix_numbers(int a, int b) {
    char text[32];
    snprintf(text, sizeof text, "%d", a * 31 + b);
    return (int)strlen(text) * 1000 + a * 31 + b;
}
