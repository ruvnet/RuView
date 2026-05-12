/* SPDX-License-Identifier: MIT
 *
 * Minimal C shim so ESP-IDF's idf_component_register has a SRCS file.
 * The real C ABI lives in src/lib.rs (Rust staticlib) and is exposed
 * through include/ruv_temporal.h.
 *
 * Intentionally empty — do not put logic here.
 */

#include "ruv_temporal.h"
