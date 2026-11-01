#ifndef RUVIEW_TEST_PSA_CRYPTO_H
#define RUVIEW_TEST_PSA_CRYPTO_H
#include <stddef.h>
#include <stdint.h>
typedef int psa_status_t;
typedef uint32_t psa_algorithm_t;
#define PSA_SUCCESS 0
#define PSA_ALG_SHA_256 ((psa_algorithm_t)0x02000009)
psa_status_t psa_crypto_init(void);
psa_status_t psa_hash_compute(psa_algorithm_t algorithm,
                            const uint8_t *input, size_t input_length,
                            uint8_t *hash, size_t hash_size, size_t *hash_length);
#endif
