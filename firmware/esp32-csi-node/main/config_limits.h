/**
 * @file config_limits.h
 * @brief Canonical acceptance limits for the NVS-backed configuration.
 *
 * The HTTP configuration API and the boot loader MUST agree on these ranges.
 * A value the API accepts but the loader rejects is the worst kind of bug in
 * this subsystem: the API answers 200, the GET endpoint reads the stored value
 * back, and the running configuration silently keeps its default. The value
 * looks applied and is not, and only a reboot reveals it.
 *
 * One definition, two consumers -- config_api.c (the API table) and
 * nvs_config.c (the loader) -- so the two cannot drift again. The host test in
 * test/test_config_rules.c pins the boundaries.
 *
 * Dependency-free on purpose: the host test includes this header without
 * ESP-IDF, exactly like the other pure headers in this firmware.
 */
#pragma once

/* Vital-sign window, in samples. The loader accepted 32..256; the API used to
 * accept 8..4096, so 8..31 and 257..4096 were stored, reported as applied and
 * then ignored at boot. */
#define CFG_LIMIT_VITAL_WINDOW_MIN  32u
#define CFG_LIMIT_VITAL_WINDOW_MAX  256u

/* Subcarriers kept per packet. The loader accepted 1..32; the API used to
 * accept 1..64, so 33..64 were stored, reported as applied and then ignored at
 * boot. */
#define CFG_LIMIT_TOP_K_MIN         1u
#define CFG_LIMIT_TOP_K_MAX         32u

/* Channels the node hops over. The loader accepts 1..NVS_CFG_HOP_MAX, which is
 * the size of the channel list; the API used to accept 1..14, so 7..14 were
 * stored, reported as applied and then ignored at boot. */
#define CFG_LIMIT_HOP_COUNT_MIN     1u
#define CFG_LIMIT_HOP_COUNT_MAX     6u
