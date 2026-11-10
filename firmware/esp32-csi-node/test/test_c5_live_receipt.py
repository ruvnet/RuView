"""Execute actual callback receipt reads, metadata construction and final fence."""
import pathlib,subprocess,tempfile,unittest
ROOT=pathlib.Path(__file__).resolve().parents[1]
class LiveReceiptTests(unittest.TestCase):
 def test_actual_callback_refuses_drift_and_link_epoch_changes(self):
  source=(ROOT/'main/csi_collector.c').read_text()
  a=source.index('static bool c5_current_phy_matches(void)');b=source.index('\n}',a)+2
  reader=source[a:b]
  link=source[source.index('void csi_collector_note_link_transition(bool connected)'):]
  a=source.index('    const unsigned callback_link_state =');b=source.index('    const int64_t native_log_now',a)
  metadata=source[a:b]
  a=source.index('    const bool closing_phy_matches =',source.index('    frame_len = c5_lltf8_v1_append'))
  b=source.index('\n#endif',a)
  fence=source[a:b].replace(')) return;', ')) return false;')
  head=r'''
#define CONFIG_C5_MANAGED_LLTF8_V1 1
#include <assert.h>
#include <stdatomic.h>
#include <stdio.h>
#include "c5_lltf8_contract.h"
#include "c5_lltf8_phy_receipt.h"
#include "c5_lltf8_live_gate.h"
static atomic_uint s_c5_link_state;
static bool s_c5_lltf8_readback=true;
static unsigned s_c5_lltf8_sta_bandwidth_mhz=20;
static uint32_t ctrl=C5_LLTF8_PHY_CTRL_EXPECTED,ack0,ack1;
static unsigned reads, mutate_at, transition;
static uint32_t read_register(uint32_t address) {
 reads++;
 if(reads==mutate_at)ctrl^=1u<<12;
 if(address==C5_LLTF8_PHY_CTRL_ADDR)return ctrl;
 if(address==C5_LLTF8_ACK0_ADDR)return ack0;
 assert(address==C5_LLTF8_ACK1_ADDR);return ack1;
}
#define REG_READ(address) read_register(address)
typedef struct {struct {unsigned cur_bb_format,second,rx_channel_estimate_info_vld;}rx_ctrl;bool first_word_invalid;unsigned len;} test_info;
'''
  wrapper='''static bool execute_actual_receipt(test_info *info) {\n'''+metadata+'''
if(!native_eligible)return false;
/* A transition while filtering/serializing must not reuse the opening receipt. */
if(transition==1){csi_collector_note_link_transition(false);}
if(transition==2){csi_collector_note_link_transition(false);csi_collector_note_link_transition(true);}
'''+fence+'''\nreturn true;}\n'''
  main=r'''
static void reset(void){ctrl=C5_LLTF8_PHY_CTRL_EXPECTED;ack0=ack1=reads=mutate_at=transition=0;atomic_store(&s_c5_link_state,3);}
int main(void){test_info info={{1,0,1},false,106};
reset();assert(execute_actual_receipt(&info));assert(reads==6);
/* Stale startup bool cannot override any changed required PHY field. */
for(unsigned bit=0;bit<32;bit++)if(C5_LLTF8_PHY_CTRL_MASK&(1u<<bit)){reset();ctrl^=1u<<bit;assert(!execute_actual_receipt(&info));}
reset();ack0=C5_LLTF8_ACK_MASK;assert(!execute_actual_receipt(&info));
reset();ack1=C5_LLTF8_ACK_MASK;assert(!execute_actual_receipt(&info));
reset();atomic_store(&s_c5_link_state,4);assert(!execute_actual_receipt(&info));
reset();transition=1;assert(!execute_actual_receipt(&info));
reset();transition=2;assert(!execute_actual_receipt(&info)); /* disconnect/reconnect ABA */
reset();mutate_at=4;assert(!execute_actual_receipt(&info)); /* mode drift at closing read */
reset();ctrl^=1u<<12;assert(!execute_actual_receipt(&info));
ctrl=C5_LLTF8_PHY_CTRL_EXPECTED;atomic_store(&s_c5_link_state,7);reads=0;assert(execute_actual_receipt(&info)); /* fresh recovered profile */
reset();info.rx_ctrl.rx_channel_estimate_info_vld=0;assert(!execute_actual_receipt(&info));
reset();info.rx_ctrl.rx_channel_estimate_info_vld=1;info.len=104;assert(!execute_actual_receipt(&info));
puts("PASS: actual callback fresh PHY, drift, disconnect, reconnect ABA, recovery and original metadata controls");return 0;}
'''
  with tempfile.TemporaryDirectory() as d:
   p=pathlib.Path(d);(p/'probe.c').write_text(head+reader+link+wrapper+main)
   subprocess.run(['cc','-std=c11','-Wall','-Wextra','-Werror','-fsanitize=address,undefined','-I'+str(ROOT/'main'),str(p/'probe.c'),'-o',str(p/'probe')],check=True)
   subprocess.run([str(p/'probe')],check=True)
if __name__=='__main__':unittest.main()
