/**
 * @file psa/crypto.h
 * @brief Minimal PSA Crypto stub for host-based unit tests.
 *
 * Provides just enough of the PSA Crypto API for device_digest() to compile
 * and run on a Windows/Linux host. SHA-256 is implemented via a small
 * public-domain C implementation embedded here.
 */
#ifndef PSA_CRYPTO_H_STUB
#define PSA_CRYPTO_H_STUB

#include <stdint.h>
#include <stddef.h>
#include <string.h>

/* ---- PSA status codes ---- */
typedef int32_t psa_status_t;
#define PSA_SUCCESS                 ((psa_status_t)0)
#define PSA_ERROR_GENERIC_ERROR     ((psa_status_t)-132)

/* ---- PSA algorithm ---- */
typedef uint32_t psa_algorithm_t;
#define PSA_ALG_SHA_256             ((psa_algorithm_t)0x02000009)

/* ---- psa_crypto_init ---- */
static inline psa_status_t psa_crypto_init(void) { return PSA_SUCCESS; }

/* ---- Minimal SHA-256 (public domain, Zack Rusin) ---- */

static const uint32_t _sha256_k[64] = {
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,
    0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,
    0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,
    0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,
    0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,
    0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,
    0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,
    0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,
    0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
};

#define _ROR32(x,n) (((x)>>(n))|((x)<<(32-(n))))
#define _CH(e,f,g)  (((e)&(f))^(~(e)&(g)))
#define _MAJ(a,b,c) (((a)&(b))^((a)&(c))^((b)&(c)))
#define _EP0(a)     (_ROR32(a,2)^_ROR32(a,13)^_ROR32(a,22))
#define _EP1(e)     (_ROR32(e,6)^_ROR32(e,11)^_ROR32(e,25))
#define _SIG0(x)    (_ROR32(x,7)^_ROR32(x,18)^((x)>>3))
#define _SIG1(x)    (_ROR32(x,17)^_ROR32(x,19)^((x)>>10))

static inline void _sha256_transform(uint32_t state[8], const uint8_t block[64])
{
    uint32_t w[64], a,b,c,d,e,f,g,h,t1,t2;
    int i;
    for (i=0;i<16;i++)
        w[i] = ((uint32_t)block[i*4]<<24)|((uint32_t)block[i*4+1]<<16)|
               ((uint32_t)block[i*4+2]<<8)|(uint32_t)block[i*4+3];
    for (;i<64;i++)
        w[i] = _SIG1(w[i-2])+w[i-7]+_SIG0(w[i-15])+w[i-16];
    a=state[0]; b=state[1]; c=state[2]; d=state[3];
    e=state[4]; f=state[5]; g=state[6]; h=state[7];
    for (i=0;i<64;i++){
        t1=h+_EP1(e)+_CH(e,f,g)+_sha256_k[i]+w[i];
        t2=_EP0(a)+_MAJ(a,b,c);
        h=g; g=f; f=e; e=d+t1; d=c; c=b; b=a; a=t1+t2;
    }
    state[0]+=a; state[1]+=b; state[2]+=c; state[3]+=d;
    state[4]+=e; state[5]+=f; state[6]+=g; state[7]+=h;
}

static inline psa_status_t psa_hash_compute(
    psa_algorithm_t alg,
    const uint8_t *input, size_t input_length,
    uint8_t *hash, size_t hash_size, size_t *hash_length)
{
    (void)alg;
    if (hash_size < 32) return PSA_ERROR_GENERIC_ERROR;

    uint32_t state[8] = {
        0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,
        0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19
    };

    uint8_t block[64];
    size_t i, n = input_length;
    const uint8_t *p = input;

    /* Process full 64-byte blocks */
    while (n >= 64) { _sha256_transform(state, p); p += 64; n -= 64; }

    /* Build final padded block(s) */
    memset(block, 0, 64);
    memcpy(block, p, n);
    block[n] = 0x80;
    if (n >= 56) {
        _sha256_transform(state, block);
        memset(block, 0, 64);
    }
    uint64_t bits = (uint64_t)input_length * 8;
    for (i=0;i<8;i++) block[63-i] = (uint8_t)(bits >> (i*8));
    _sha256_transform(state, block);

    for (i=0;i<8;i++){
        hash[i*4+0]=(uint8_t)(state[i]>>24);
        hash[i*4+1]=(uint8_t)(state[i]>>16);
        hash[i*4+2]=(uint8_t)(state[i]>>8);
        hash[i*4+3]=(uint8_t)(state[i]);
    }
    *hash_length = 32;
    return PSA_SUCCESS;
}

#undef _ROR32
#undef _CH
#undef _MAJ
#undef _EP0
#undef _EP1
#undef _SIG0
#undef _SIG1

#endif /* PSA_CRYPTO_H_STUB */
