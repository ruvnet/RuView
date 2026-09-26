/**
 * @file device_digest_shim.h
 * @brief Host-test declaration for the device_digest shim.
 */
#ifndef DEVICE_DIGEST_SHIM_H
#define DEVICE_DIGEST_SHIM_H

/**
 * Compute the 16-hex-char device digest into output[17].
 * Mirrors the static device_digest() in serial_onboarding.c.
 */
void device_digest(char output[17]);

#endif /* DEVICE_DIGEST_SHIM_H */
