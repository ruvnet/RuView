/* Standalone sanitizer probe. Synthetic arrays only; no model or sensing input. */
#include <float.h>
#include <math.h>
#include <stddef.h>
#include <stdio.h>

unsigned int espfi_motion_abi(void);
int espfi_motion_features(const double *, size_t, double *, size_t);

#define CHECK(condition) do { if (!(condition)) { \
    fprintf(stderr, "native probe failed at line %d\n", __LINE__); return 1; \
} } while (0)

int main(void) {
    double input[9880] = {0};
    double guarded[470] = {0};
    double *output = guarded + 1;
    guarded[0] = 12345.0;
    guarded[469] = -12345.0;
    CHECK(espfi_motion_abi() == 1u);
    CHECK(espfi_motion_features(NULL, 9880, output, 468) != 0);
    CHECK(espfi_motion_features(input, 9880, NULL, 468) != 0);
    CHECK(espfi_motion_features(input, 9879, output, 468) != 0);
    CHECK(espfi_motion_features(input, 9881, output, 468) != 0);
    CHECK(espfi_motion_features(input, (size_t)-1, output, 468) != 0);
    CHECK(espfi_motion_features(input, 9880, output, 467) != 0);
    CHECK(espfi_motion_features(input, 9880, output, 469) != 0);
    CHECK(espfi_motion_features(input, 9880, output, 468) == 0);
    for (size_t i = 0; i < 468; i++) CHECK(output[i] == 0.0);
    input[0] = NAN;
    CHECK(espfi_motion_features(input, 9880, output, 468) != 0);
    input[0] = INFINITY;
    CHECK(espfi_motion_features(input, 9880, output, 468) != 0);
    input[0] = DBL_MAX;
    CHECK(espfi_motion_features(input, 9880, output, 468) != 0);
    for (size_t i = 0; i < 9880; i++) input[i] = i % 2 ? 1e-6 : -1e-6;
    CHECK(espfi_motion_features(input, 9880, output, 468) == 0);
    for (size_t c = 0; c < 52; c++) {
        CHECK(fabs(output[c*9 + 6] - 2.0) < 1e-10);
        CHECK(output[c*9 + 7] == 0.0);
        CHECK(output[c*9 + 8] == 0.0);
    }
    for (size_t i = 0; i < 468; i++) CHECK(isfinite(output[i]));
    for (size_t i = 0; i < 9880; i++) CHECK(input[i] == (i % 2 ? 1e-6 : -1e-6));
    CHECK(guarded[0] == 12345.0 && guarded[469] == -12345.0);
    puts("native sanitizer probe passed");
    return 0;
}
