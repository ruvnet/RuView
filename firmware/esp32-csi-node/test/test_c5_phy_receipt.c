#include <assert.h>
#include "c5_lltf8_phy_receipt.h"
int main(void)
{
    const uint32_t good=C5_LLTF8_PHY_CTRL_EXPECTED;
    assert(c5_lltf8_phy_receipt_matches(good,0,0));
    /* Every required bit independently changes admission. */
    for(unsigned i=0;i<32;i++) {
        const uint32_t bit=UINT32_C(1)<<i;
        assert(c5_lltf8_phy_receipt_matches(good^bit,0,0)==((C5_LLTF8_PHY_CTRL_MASK&bit)==0));
        assert(c5_lltf8_phy_receipt_matches(good,bit,0)==((C5_LLTF8_ACK_MASK&bit)==0));
        assert(c5_lltf8_phy_receipt_matches(good,0,bit)==((C5_LLTF8_ACK_MASK&bit)==0));
    }
    /* Bit2 has deliberately NO interpretation; unrelated register state is
     * neither cleared nor falsely required to match a synthetic full word. */
    assert(c5_lltf8_phy_receipt_matches(good|4,0,0));
    assert(!c5_lltf8_phy_receipt_matches(0,0,0));
    return 0;
}
