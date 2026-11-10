/* Private ESP-IDF 6.0.2/C5 binary-derived read-only control receipt.
 * NOT a portable register API, gain calibration or hardware attestation.
 * See C5-LLTF8-CONTRACT-V1.md for exact SDK/library pins and instruction proof.
 * Unrelated bits (especially PHY bit2) have no interpretation here. */
#ifndef RUVIEW_C5_LLTF8_PHY_RECEIPT_H
#define RUVIEW_C5_LLTF8_PHY_RECEIPT_H
#include <stdbool.h>
#include <stdint.h>
#define C5_LLTF8_PHY_CTRL_ADDR UINT32_C(0x600a790c)
#define C5_LLTF8_ACK0_ADDR UINT32_C(0x600a40fc)
#define C5_LLTF8_ACK1_ADDR UINT32_C(0x600a4100)
#define C5_LLTF8_PHY_CTRL_MASK UINT32_C(0x30f8)
#define C5_LLTF8_PHY_CTRL_EXPECTED UINT32_C(0x3010)
#define C5_LLTF8_ACK_MASK UINT32_C(0x20000000)
static inline bool c5_lltf8_phy_receipt_matches(uint32_t ctrl, uint32_t ack0, uint32_t ack1)
{
    return (ctrl & C5_LLTF8_PHY_CTRL_MASK) == C5_LLTF8_PHY_CTRL_EXPECTED &&
           (ack0 & C5_LLTF8_ACK_MASK) == 0 &&
           (ack1 & C5_LLTF8_ACK_MASK) == 0;
}
#endif
