/**
 * @file rvf_parser.h
 * @brief RVF (RuVector Format) container parser for WASM sensing modules.
 *
 * RVF wraps a WASM binary with a manifest (capabilities, budgets, schema),
 * an Ed25519 signature, and optional test vectors.  The ESP32 never accepts
 * raw .wasm over HTTP when wasm_verify is enabled — only signed RVF.
 *
 * Binary layout (all fields little-endian):
 *
 *   [Header: 32 bytes] [Manifest: 96 bytes] [WASM payload: N bytes]
 *   [Ed25519 signature: 0 or 64 bytes] [Test vectors: M bytes]
 *
 * Signature covers bytes 0 through (header + manifest + wasm - 1).
 */

#ifndef RVF_PARSER_H
#define RVF_PARSER_H

#include <stdint.h>
#include <stdbool.h>
#include "esp_err.h"

/* ---- Magic and version ---- */
#define RVF_MAGIC           0x01465652  /**< "RVF\x01" as u32 LE. */
#define RVF_FORMAT_VERSION  1
#define RVF_HEADER_SIZE     32
#define RVF_MANIFEST_SIZE   96
#define RVF_HOST_API_V1     1
#define RVF_SIGNATURE_LEN   64  /**< Ed25519 signature length. */

/* Raw WASM magic (for fallback detection). */
#define WASM_BINARY_MAGIC   0x6D736100  /**< "\0asm" as u32 LE. */

/* ---- Capability bitmask ---- */
#define RVF_CAP_READ_PHASE     (1 << 0)  /**< csi_get_phase */
#define RVF_CAP_READ_AMPLITUDE (1 << 1)  /**< csi_get_amplitude */
#define RVF_CAP_READ_VARIANCE  (1 << 2)  /**< csi_get_variance */
#define RVF_CAP_READ_VITALS    (1 << 3)  /**< csi_get_bpm_*, presence, persons */
#define RVF_CAP_READ_HISTORY   (1 << 4)  /**< csi_get_phase_history */
#define RVF_CAP_EMIT_EVENTS    (1 << 5)  /**< csi_emit_event */
#define RVF_CAP_LOG            (1 << 6)  /**< csi_log */
#define RVF_CAP_ALL            0x7F

/* ---- Header flags ---- */
#define RVF_FLAG_HAS_SIGNATURE    (1 << 0)
#define RVF_FLAG_HAS_TEST_VECTORS (1 << 1)

/* ---- Header (32 bytes, packed) ---- */
typedef struct __attribute__((packed)) {
    uint32_t magic;             /**< RVF_MAGIC. */
    uint16_t format_version;    /**< RVF_FORMAT_VERSION. */
    uint16_t flags;             /**< RVF_FLAG_* bitmask. */
    uint32_t manifest_len;      /**< Always RVF_MANIFEST_SIZE. */
    uint32_t wasm_len;          /**< WASM payload size in bytes. */
    uint32_t signature_len;     /**< 0 or RVF_SIGNATURE_LEN. */
    uint32_t test_vectors_len;  /**< 0 if no test vectors. */
    uint32_t total_len;         /**< Sum of all sections. */
    uint32_t reserved;          /**< Must be 0. */
} rvf_header_t;

_Static_assert(sizeof(rvf_header_t) == RVF_HEADER_SIZE, "RVF header must be 32 bytes");

/* ---- Manifest (96 bytes, packed) ---- */
typedef struct __attribute__((packed)) {
    char     module_name[32];       /**< Null-terminated ASCII name. */
    uint16_t required_host_api;     /**< RVF_HOST_API_V1. */
    uint32_t capabilities;          /**< RVF_CAP_* bitmask. */
    uint32_t max_frame_us;          /**< Requested budget per on_frame (0 = use default). */
    uint16_t max_events_per_sec;    /**< Rate limit (0 = unlimited). */
    uint16_t memory_limit_kb;       /**< Max WASM heap requested (0 = use default). */
    uint16_t event_schema_version;  /**< For receiver compatibility. */
    uint8_t  build_hash[32];        /**< SHA-256 of WASM payload. */
    uint16_t min_subcarriers;       /**< Minimum required (0 = any). */
    uint16_t max_subcarriers;       /**< Maximum expected (0 = any). */
    char     author[10];            /**< Null-padded ASCII. */
    uint8_t  _reserved[2];         /**< Pad to 96 bytes. */
} rvf_manifest_t;

_Static_assert(sizeof(rvf_manifest_t) == RVF_MANIFEST_SIZE, "RVF manifest must be 96 bytes");

/* ---- Parse result ---- */
typedef struct {
    const rvf_header_t   *header;       /**< Points into input buffer. */
    const rvf_manifest_t *manifest;     /**< Points into input buffer. */
    const uint8_t        *wasm_data;    /**< Points to WASM payload. */
    uint32_t              wasm_len;     /**< WASM payload length. */
    const uint8_t        *signature;    /**< Points to signature (or NULL). */
    const uint8_t        *test_vectors; /**< Points to test vectors (or NULL). */
    uint32_t              test_vectors_len;
} rvf_parsed_t;

/**
 * Parse an RVF container from a byte buffer.
 *
 * Validates header magic, version, sizes, and SHA-256 build hash.
 * Does NOT verify the Ed25519 signature (call rvf_verify_signature separately).
 *
 * @param data     Input buffer containing the full RVF.
 * @param data_len Length of the input buffer.
 * @param out      Parsed result with pointers into the input buffer.
 * @return ESP_OK if structurally valid.
 */
esp_err_t rvf_parse(const uint8_t *data, uint32_t data_len, rvf_parsed_t *out);

/**
 * Verify the Ed25519 signature of an RVF.
 *
 * @param parsed   Result from rvf_parse().
 * @param data     Original input buffer.
 * @param pubkey   32-byte Ed25519 public key.
 * @return ESP_OK if signature is valid.
 */
esp_err_t rvf_verify_signature(const rvf_parsed_t *parsed, const uint8_t *data,
                                const uint8_t *pubkey);

/**
 * Check if a buffer starts with the RVF magic.
 *
 * @param data     Input buffer (at least 4 bytes).
 * @param data_len Length of the buffer.
 * @return true if the buffer starts with "RVF\x01".
 */
bool rvf_is_rvf(const uint8_t *data, uint32_t data_len);

/**
 * Check if a buffer starts with raw WASM magic ("\0asm").
 *
 * @param data     Input buffer (at least 4 bytes).
 * @param data_len Length of the buffer.
 * @return true if the buffer starts with WASM binary magic.
 */
bool rvf_is_raw_wasm(const uint8_t *data, uint32_t data_len);

#endif /* RVF_PARSER_H */
