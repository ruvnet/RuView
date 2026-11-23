/**
 * @file csi_he_gate.h
 * @brief ADR-423 D1: per-frame HE-LTF validity gate (pure logic, host-testable).
 *
 * On HE-capable targets (ESP32-C5/C6) the Wi-Fi driver reports
 * rx_ctrl.rx_channel_estimate_info_vld per CSI frame. Measured on an ESP32-C6
 * (ADR-423 §B, 2026-11-09): HE-SU frames with vld=0 changed shape 51.6%
 * frame-to-frame versus 3.2% for vld=1 (same transmitter, gain-normalised),
 * and 77-90% of HE-SU frames were vld=0. Feeding them to edge DSP pollutes
 * presence, vitals and top-K selection.
 *
 * The gate only acts on HE PPDUs (cur_bb_format >= 4). Non-HE frames on the
 * same targets were not shown to be unstable, so they pass unchanged.
 */
#ifndef CSI_HE_GATE_H
#define CSI_HE_GATE_H

#include <stdbool.h>
#include <stdint.h>

/** Policy for HE frames whose channel estimate is flagged invalid. */
typedef enum {
    CSI_HE_POLICY_DROP = 0, /**< Discard before rate gate, stream and edge DSP. */
    CSI_HE_POLICY_TAG  = 1, /**< Stream with ADR-018 byte 19 bit 5 set; skip edge DSP. */
    CSI_HE_POLICY_PASS = 2, /**< Pre-ADR-423 behaviour (A/B work only). */
} csi_he_policy_t;

/** What the callback must do with one frame. */
typedef enum {
    CSI_HE_ACTION_ACCEPT = 0, /**< Normal path: stream + edge DSP. */
    CSI_HE_ACTION_DROP   = 1, /**< Return immediately; count as invalid. */
    CSI_HE_ACTION_TAG    = 2, /**< Stream with invalid bit; do not enqueue to edge DSP. */
} csi_he_action_t;

/** ADR-018 byte 19 flag: HE channel estimate flagged invalid by the driver. */
#define CSI_FRAME_FLAG_HE_ESTIMATE_INVALID (1u << 5)

/** cur_bb_format values >= this are HE PPDUs (4=HE-SU, 5=HE-MU, 6=HE-ERSU, 7=HE-TB). */
#define CSI_BB_FORMAT_HE_MIN 4u

static inline bool csi_he_frame_is_invalid(unsigned cur_bb_format, bool estimate_valid)
{
    return cur_bb_format >= CSI_BB_FORMAT_HE_MIN && !estimate_valid;
}

static inline csi_he_action_t csi_he_gate_action(unsigned cur_bb_format,
                                                 bool estimate_valid,
                                                 csi_he_policy_t policy)
{
    if (!csi_he_frame_is_invalid(cur_bb_format, estimate_valid)) {
        return CSI_HE_ACTION_ACCEPT;
    }
    switch (policy) {
        case CSI_HE_POLICY_TAG:  return CSI_HE_ACTION_TAG;
        case CSI_HE_POLICY_PASS: return CSI_HE_ACTION_ACCEPT;
        case CSI_HE_POLICY_DROP:
        default:                 return CSI_HE_ACTION_DROP; /* fail closed */
    }
}

#endif /* CSI_HE_GATE_H */
