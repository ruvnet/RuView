import { python } from './helpers/python.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, linkSync, symlinkSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const script=fileURLToPath(new URL('../../../scripts/esp32_jsonl_to_rvcsi.py',import.meta.url));
for(const alias of ['identical path','hard link','symbolic link']){
 test(`public converter CLI preserves source capture when output is ${alias}`,t=>{
  const dir=mkdtempSync(join(tmpdir(),'ruview-transcode-source-'));
  try{
   const input=join(dir,'input.csi.jsonl');let output=input;
   const original=JSON.stringify({type:'raw_csi',iq_hex:'0304',rssi:-50,ts_ns:123,channel:6})+'\n';writeFileSync(input,original);
   if(alias!=='identical path'){
    output=join(dir,'output.rvcsi');
    if(alias==='hard link')linkSync(input,output);
    else {
     try { symlinkSync(input,output); }
     catch(error) {
      if(process.platform==='win32' && error.code==='EPERM'){
       t.skip('Windows account lacks file symlink privilege'); return;
      }
      throw error;
     }
    }
   }
   const r=spawnSync(python,[script,input,output],{encoding:'utf8'});
   assert.equal(readFileSync(input,'utf8'),original,'input capture must remain byte-for-byte intact');
   assert.equal(r.status,2);assert.match(r.stderr,/different files/i);
  }finally{rmSync(dir,{recursive:true,force:true});}
 });
}
