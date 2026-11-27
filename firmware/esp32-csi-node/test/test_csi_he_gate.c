/* ADR-423 D1: HE-LTF validity gate decision table (pure logic, SYNTHETIC).
 * Verifies the policy contract only; the measured justification for the gate
 * is the hardware bench in ADR-423 §B, not this test. */
#include "csi_he_gate.h"
#include <assert.h>
#include <stdio.h>

int main(void)
{
    const csi_he_policy_t policies[] = {CSI_HE_POLICY_DROP, CSI_HE_POLICY_TAG, CSI_HE_POLICY_PASS};

    /* Non-HE formats (11b, 11g, HT, VHT) always pass, whatever the flag says:
     * the gate was only measured on HE PPDUs. */
    for (unsigned bb = 0; bb < CSI_BB_FORMAT_HE_MIN; bb++) {
        for (unsigned p = 0; p < 3; p++) {
            assert(csi_he_gate_action(bb, false, policies[p]) == CSI_HE_ACTION_ACCEPT);
            assert(csi_he_gate_action(bb, true, policies[p]) == CSI_HE_ACTION_ACCEPT);
            assert(!csi_he_frame_is_invalid(bb, false));
        }
    }

    /* HE-SU/MU/ER-SU/TB with a valid estimate always pass. */
    for (unsigned bb = CSI_BB_FORMAT_HE_MIN; bb <= 7; bb++) {
        for (unsigned p = 0; p < 3; p++) {
            assert(csi_he_gate_action(bb, true, policies[p]) == CSI_HE_ACTION_ACCEPT);
        }
        assert(csi_he_frame_is_invalid(bb, false));
        /* Invalid HE estimate: the policy decides. */
        assert(csi_he_gate_action(bb, false, CSI_HE_POLICY_DROP) == CSI_HE_ACTION_DROP);
        assert(csi_he_gate_action(bb, false, CSI_HE_POLICY_TAG) == CSI_HE_ACTION_TAG);
        assert(csi_he_gate_action(bb, false, CSI_HE_POLICY_PASS) == CSI_HE_ACTION_ACCEPT);
    }

    /* An out-of-range policy value fails closed (drop), never open. */
    assert(csi_he_gate_action(4, false, (csi_he_policy_t)99) == CSI_HE_ACTION_DROP);

    /* The tag bit must not collide with ADR-110 byte-19 flags
     * (bit0 bw40, bit2 STBC, bit3 LDPC, bit4 cross-node sync). */
    assert((CSI_FRAME_FLAG_HE_ESTIMATE_INVALID & ((1u << 0) | (1u << 2) | (1u << 3) | (1u << 4))) == 0);

    puts("PASS: ADR-423 HE validity gate decision table");
    return 0;
}
