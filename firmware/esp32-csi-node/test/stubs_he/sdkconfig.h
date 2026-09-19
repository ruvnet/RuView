/* Stub: sdkconfig.h for an HE-capable target (ESP32-C6/C5).
 *
 * Exists so the subcarrier-grid test can prove that edge_processing.h pulls in
 * sdkconfig.h ITSELF, rather than relying on some other header having done it
 * first. The macro is deliberately defined ONLY here and never on the compiler
 * command line: if the header stops including sdkconfig.h, C evaluates the
 * undefined identifier in `#if` as 0, silently selects the 128-bin pre-HE grid
 * on a 256-bin part, and the whole edge pipeline goes quiet with no warning.
 * Passing -DCONFIG_SOC_WIFI_HE_SUPPORT=1 instead would mask exactly that.
 *
 * Placed on the include path ahead of stubs/ so it shadows the base stub.
 */
#ifndef SDKCONFIG_H_STUB
#define SDKCONFIG_H_STUB

#include "esp_stubs.h"

/* The SoC capability macro IDF defines for HE-capable parts. */
#define CONFIG_SOC_WIFI_HE_SUPPORT 1

#endif
