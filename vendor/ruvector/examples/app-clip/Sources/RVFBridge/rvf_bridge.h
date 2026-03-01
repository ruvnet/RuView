/*
 * rvf_bridge.h — C header declaring the RVF FFI functions for the App Clip.
 *
 * These declarations mirror the extern "C" functions exported by
 * crates/rvf/rvf-runtime/src/ffi.rs. The App Clip calls these through
 * the pre-built librvf_runtime.a static library.
 */

#ifndef RVF_BRIDGE_H
#define RVF_BRIDGE_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Result codes ---- */

#define RVQS_OK                    0
#define RVQS_ERR_NULL_PTR         -1
#define RVQS_ERR_TOO_SHORT        -2
#define RVQS_ERR_BAD_MAGIC        -3
#define RVQS_ERR_SIGNATURE_INVALID -4
#define RVQS_ERR_HASH_MISMATCH    -5
#define RVQS_ERR_DECOMPRESS_FAIL  -6
#define RVQS_ERR_BUFFER_TOO_SMALL -7
#define RVQS_ERR_PARSE_FAIL       -8

/* ---- Structs ---- */

/**
 * Mirrors the RvqsHeaderC struct from ffi.rs.
 * 64-byte fixed-size header of an RVQS QR Cognitive Seed.
 */
typedef struct {
    uint32_t seed_magic;
    uint16_t seed_version;
    uint16_t flags;
    uint8_t  file_id[8];
    uint32_t total_vector_count;
    uint16_t dimension;
    uint8_t  base_dtype;
    uint8_t  profile_id;
    uint64_t created_ns;
    uint32_t microkernel_offset;
    uint32_t microkernel_size;
    uint32_t download_manifest_offset;
    uint32_t download_manifest_size;
    uint16_t sig_algo;
    uint16_t sig_length;
    uint32_t total_seed_size;
    uint8_t  content_hash[8];
} RvqsHeaderC;

/**
 * High-level seed parse result returned to Swift.
 * Populated by rvf_seed_parse and freed by rvf_seed_free.
 */
typedef struct {
    /** Seed format version. */
    uint16_t version;
    /** Number of download hosts in the manifest. */
    uint32_t host_count;
    /** Number of progressive layers in the manifest. */
    uint32_t layer_count;
    /** SHAKE-256-64 content hash (8 bytes). */
    uint8_t  content_hash[8];
    /** Total vector count from the header. */
    uint32_t total_vector_count;
    /** Vector dimensionality. */
    uint16_t dimension;
    /** Total seed payload size. */
    uint32_t total_seed_size;
    /** Seed flags bitfield. */
    uint16_t flags;
} RvfSeedResult;

/* ---- FFI Functions (from librvf_runtime.a) ---- */

/**
 * Parse a raw RVQS seed payload and extract header information.
 *
 * @param data    Pointer to the raw QR seed bytes.
 * @param len     Length of the data buffer.
 * @param out     Pointer to an RvqsHeaderC struct to receive the parsed header.
 * @return RVQS_OK on success, or a negative error code.
 */
int32_t rvqs_parse_header(const uint8_t *data, size_t len, RvqsHeaderC *out);

/**
 * Verify the HMAC-SHA256 signature of a QR seed.
 *
 * @param data     Pointer to the full seed payload.
 * @param data_len Length of the seed payload.
 * @param key      Pointer to the signing key.
 * @param key_len  Length of the signing key.
 * @return RVQS_OK if signature is valid, or a negative error code.
 */
int32_t rvqs_verify_signature(const uint8_t *data, size_t data_len,
                              const uint8_t *key, size_t key_len);

/**
 * Verify the content hash of a QR seed payload.
 *
 * @param data     Pointer to the full seed payload.
 * @param data_len Length of the seed payload.
 * @return RVQS_OK if hash matches, or a negative error code.
 */
int32_t rvqs_verify_content_hash(const uint8_t *data, size_t data_len);

/**
 * Decompress the WASM microkernel from a QR seed.
 *
 * @param data     Pointer to the full seed payload.
 * @param data_len Length of the seed payload.
 * @param out      Buffer to receive decompressed microkernel.
 * @param out_cap  Capacity of the output buffer.
 * @param out_len  Receives the actual decompressed size.
 * @return RVQS_OK on success, or a negative error code.
 */
int32_t rvqs_decompress_microkernel(const uint8_t *data, size_t data_len,
                                    uint8_t *out, size_t out_cap,
                                    size_t *out_len);

/**
 * Extract the primary host URL from the download manifest.
 *
 * @param data     Pointer to the full seed payload.
 * @param data_len Length of the seed payload.
 * @param url_buf  Buffer to receive the URL string (not null-terminated).
 * @param url_cap  Capacity of the URL buffer.
 * @param url_len  Receives the actual URL length.
 * @return RVQS_OK on success, or a negative error code.
 */
int32_t rvqs_get_primary_host_url(const uint8_t *data, size_t data_len,
                                  uint8_t *url_buf, size_t url_cap,
                                  size_t *url_len);

/* ---- Convenience wrappers (implemented in Swift, declared here for reference) ---- */

/**
 * Parse a QR seed payload into a high-level RvfSeedResult.
 *
 * This is a convenience wrapper that calls rvqs_parse_header internally
 * and populates the simplified result struct. Implemented on the Swift side
 * using the lower-level FFI functions above.
 *
 * @param data Pointer to the raw QR seed bytes.
 * @param len  Length of the data buffer.
 * @param out  Pointer to an RvfSeedResult struct to populate.
 * @return RVQS_OK on success, or a negative error code.
 */
int32_t rvf_seed_parse(const uint8_t *data, size_t len, RvfSeedResult *out);

/**
 * Free any resources associated with an RvfSeedResult.
 *
 * Currently a no-op since RvfSeedResult is a plain value type,
 * but provided for forward-compatibility if the struct gains
 * heap-allocated fields.
 *
 * @param result Pointer to the result to free.
 */
void rvf_seed_free(RvfSeedResult *result);

#ifdef __cplusplus
}
#endif

#endif /* RVF_BRIDGE_H */
