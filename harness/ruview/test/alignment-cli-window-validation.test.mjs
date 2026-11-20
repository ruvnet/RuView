import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const script=fileURLToPath(new URL('../../../scripts/align-ground-truth.js',import.meta.url));
function alignment(extra, inspect) {
 const dir=mkdtempSync(join(tmpdir(),'ruview-align-args-'));
 try {
  const gt=join(dir,'gt.jsonl'),csi=join(dir,'csi.jsonl'),output=join(dir,'paired.jsonl');
  const t=1750000000;
  writeFileSync(gt,[0,.01,.02,.03].map(d=>JSON.stringify({ts_ns:Math.round((t+d)*1e9),keypoints:Array.from({length:17},()=>[.5,.5]),confidence:1,n_visible:17})).join('\n')+'\n');
  writeFileSync(csi,[0,.01,.02,.03].map(d=>JSON.stringify({type:'feature',timestamp:t+d,node_id:1,features:[1,2,3,4,5,6,7,8]})).join('\n')+'\n');
  const flags=extra.filter((_,i)=>i%2===0).map((flag,i)=>`${flag}=${extra[i*2+1]}`);
  const r=spawnSync(process.execPath,['--max-old-space-size=32',script,'--gt',gt,'--csi',csi,'--output',output,'--window-frames','2','--min-camera-frames','1',...flags],{encoding:'utf8',timeout:1000,maxBuffer:128*1024});
  return inspect(r,output);
 } finally {rmSync(dir,{recursive:true,force:true});}
}
for(const [flag,value] of [['window-frames','0'],['window-frames','-1'],['window-frames','1.5'],['window-frames','2junk'],['window-ms','NaN'],['window-ms','-1'],['min-camera-frames','0'],['min-camera-frames','1.5'],['min-confidence','NaN'],['min-confidence','1.1'],['clock-offset-ms','Infinity']]){
 test(`public alignment CLI rejects --${flag}=${value} before writing output`,()=>alignment(['--'+flag,value],(r,output)=>{
  assert.equal(r.status,2,`${r.error?.message||''} ${r.stderr.slice(0,200)}`);assert.match(r.stderr,new RegExp('--'+flag));assert.equal(existsSync(output),false);
 }));
}
test('public alignment CLI retains complete windows and permits zero expansion/confidence with signed finite offset',()=>alignment(['--window-ms','0','--min-confidence','0','--clock-offset-ms','-0.5'],(r,output)=>{
 assert.equal(r.status,0,r.stderr);const frames=readFileSync(output,'utf8').trim().split('\n').map(JSON.parse);assert.equal(frames.length,2);assert.deepEqual(frames[0].csi_shape,[2,8]);assert.equal(frames[0].conf,1);assert.ok(frames.every(f=>f.n_camera_frames>0));
}));
