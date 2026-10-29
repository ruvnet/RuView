(function(root){
'use strict';
const MAX_BYTES=10*1024*1024, MAX_RECORDS=10000;
const fields='schema source_kind source_id observed_monotonic_ns run slot_index sample_start sample_rate_hz center_frequency_hz config_verified valid invalid_reasons sample_count rms_dbfs rail_fraction missing_samples event_count overrun_slices ttl_ms'.split(' ');
const modes={synthetic:'SYNTHETIC',live:'LIVE UNVERIFIED',captured_replay:'CAPTURED REPLAY'};
function validate(r){
 const fail=(s)=>{throw Error(s)};
 if(!r||Array.isArray(r)||typeof r!=='object')fail('Expected an object');
 for(const k of Object.keys(r))if(!fields.includes(k))fail('Unsupported field: '+k);
 for(const k of fields)if(!Object.hasOwn(r,k))fail('Missing field: '+k);
 if(r.schema!=='ruview.rf_diagnostics.v1')fail('Unsupported schema');
 if(!Object.hasOwn(modes,r.source_kind)||r.source_id!=='w1700k')fail('Unsupported source');
 for(const k of ['observed_monotonic_ns','run','slot_index','sample_rate_hz','sample_count','missing_samples','event_count','overrun_slices','ttl_ms'])
  if(!Number.isSafeInteger(r[k])||r[k]<0)fail('Invalid '+k);
 if(r.ttl_ms!==1000)fail('Invalid rate or TTL');
 for(const k of ['sample_start','center_frequency_hz'])
  if(r[k]!==null&&(!Number.isSafeInteger(r[k])||r[k]<0))fail('Invalid '+k);
 if(r.rms_dbfs!==null&&(typeof r.rms_dbfs!=='number'||!Number.isFinite(r.rms_dbfs)))fail('Invalid RMS');
 if(r.rail_fraction!==null&&(typeof r.rail_fraction!=='number'||!Number.isFinite(r.rail_fraction)||r.rail_fraction<0||r.rail_fraction>1))fail('Invalid rail fraction');
 if(r.config_verified!==false||r.valid!==false)fail('v1 must remain unverified and invalid');
 if(!Array.isArray(r.invalid_reasons)||r.invalid_reasons.length>32||!r.invalid_reasons.includes('configuration_unverified')||r.invalid_reasons.some(x=>typeof x!=='string'||!/^[a-z_]{1,64}$/.test(x)))fail('Invalid reasons');
 if(![0,40000000,80000000,160000000,320000000].includes(r.sample_rate_hz))fail('Invalid sample rate');
 if(r.sample_start!==null)fail('Absolute sample start is unavailable in v1');
 if(r.center_frequency_hz!==null&&(r.center_frequency_hz<2000000000||r.center_frequency_hz>8000000000))fail('Invalid centre frequency');
 if(![0,2048].includes(r.sample_count)||r.missing_samples%2048!==0)fail('Invalid sample count');
 if(r.rms_dbfs!==null&&(r.rms_dbfs< -200||r.rms_dbfs>16||r.rail_fraction===null))fail('Invalid RMS or missing rail fraction');
 const reasons=['configuration_unverified','lost','event','overrun','stale','tag_mismatch','run_change'];
 if(new Set(r.invalid_reasons).size!==r.invalid_reasons.length||r.invalid_reasons.length>7||r.invalid_reasons.some(x=>!reasons.includes(x)))fail('Unsupported invalid reason');
 return r;
}
function parse(text){
 if(typeof text!=='string'||new TextEncoder().encode(text).length>MAX_BYTES)throw Error('File exceeds 10 MiB');
 const lines=text.split(/\r?\n/).filter(x=>x.trim());
 if(!lines.length||lines.length>MAX_RECORDS)throw Error('Expected 1 to 10,000 records');
 return lines.map((s,i)=>{try{return validate(JSON.parse(s))}catch(e){throw Error('Record '+(i+1)+': '+e.message)}});
}
function demo(){return Array.from({length:96},(_,i)=>({
 schema:'ruview.rf_diagnostics.v1',source_kind:'synthetic',source_id:'w1700k',
 observed_monotonic_ns:i*100000000,run:1,slot_index:i,sample_start:null,
 sample_rate_hz:40000000,center_frequency_hz:2437000000,config_verified:false,valid:false,
 invalid_reasons:i%23===0?['configuration_unverified','lost']:['configuration_unverified'],
 sample_count:2048,rms_dbfs:-34+Math.sin(i/8)*6,rail_fraction:i%23===0?0.03:0,
 missing_samples:i%23===0?2048:0,event_count:0,overrun_slices:0,ttl_ms:1000}));}
const api={MAX_BYTES,MAX_RECORDS,modes,validate,parse,demo};
if(typeof module==='object')module.exports=api;else root.RFDiagnostics=api;
})(globalThis);
