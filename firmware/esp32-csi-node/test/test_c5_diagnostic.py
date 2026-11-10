"""Execute actual persistent formatter with startup-state and cadence controls."""
import pathlib, subprocess, tempfile, unittest
ROOT=pathlib.Path(__file__).resolve().parents[1]
class DiagnosticTests(unittest.TestCase):
 def test_actual_formatter_is_bounded_and_preserves_snapshot(self):
  src=(ROOT/'main/csi_collector.c').read_text()
  start=src.index('void csi_collector_diagnostic_tick(void)')
  fn=src[start:]
  fields=['enable','acquire_csi_legacy','acquire_csi_force_lltf','lltf_bit_mode','acquire_csi_ht20','acquire_csi_ht40','acquire_csi_vht','acquire_csi_su','acquire_csi_mu','acquire_csi_dcm','acquire_csi_beamformed','dump_ack_en','val_scale_cfg','acquire_csi_he_stbc_mode']
  decl='typedef struct {'+''.join('unsigned '+x+';' for x in fields)+'} wifi_csi_config_t;\n'
  state=src[src.index('static struct {\n    unsigned phase;'):src.index('static int64_t s_c5_diag_last_log_us;')+len('static int64_t s_c5_diag_last_log_us;')]
  head='''#include "c5_lltf8_phy_receipt.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <assert.h>
#include <string.h>
#include <stdarg.h>
static int64_t now; static unsigned logs;
static int64_t esp_timer_get_time(void){return now;}
static char output[2048];
static void capture(const char *format,...) { va_list a;va_start(a,format);int n=vsnprintf(output+strlen(output),sizeof(output)-strlen(output),format,a);va_end(a);assert(n>0);logs++; }
#define ESP_LOGI(tag,...) capture(__VA_ARGS__)
static unsigned s_c5_lltf8_sta_bandwidth_mhz=20;
static bool s_c5_lltf8_readback=false;
'''
  values=''.join('s_c5_diag.applied.'+x+'='+str(i+1)+';' for i,x in enumerate(fields))
  tail='''int main(void){s_c5_diag.phase=3;s_c5_diag.get_result=0;s_c5_diag.bandwidth_result=0;'''+values+'''
now=4999999;csi_collector_diagnostic_tick();assert(logs==0);
now=5000000;csi_collector_diagnostic_tick();
#ifdef CONFIG_C5_MANAGED_LLTF8_V1
assert(logs==3);assert(strstr(output,"phase=3")&&strstr(output,"matched=0")&&strstr(output,"register=-1 enable_result=-1"));
assert(strstr(output,"enable=1 legacy=2 force=3 unwritten_bits_mode=4 ht20=5 ht40=6 vht=7 su=8 mu=9 dcm=10 beamformed=11 unwritten_dump_ack=12 unwritten_scale=13 he_stbc=14"));
assert(strstr(output,"origin=pinned-v602-binary")&&strstr(output,"unwritten_bits_mode=4"));assert(strlen(output)<1024);now=9999999;csi_collector_diagnostic_tick();assert(logs==3);
output[0]=0;now=10000000;csi_collector_diagnostic_tick();assert(logs==6);
assert(s_c5_diag.phase==3&&s_c5_diag.register_result==-1&&s_c5_diag.enable_result==-1);
#else
assert(logs==0);
#endif
return 0;}
'''
  with tempfile.TemporaryDirectory() as d:
   p=pathlib.Path(d);(p/'p.c').write_text(head+decl+state+fn+tail)
   for flags in [[],['-DCONFIG_C5_MANAGED_LLTF8_V1=1']]:
    subprocess.run(['cc','-std=c11',*flags,'-I'+str(ROOT/'main'),str(p/'p.c'),'-o',str(p/'probe')],check=True)
    subprocess.run([str(p/'probe')],check=True)
if __name__=='__main__':unittest.main()
