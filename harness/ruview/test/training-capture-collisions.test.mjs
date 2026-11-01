import { python } from './helpers/python.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import dgram from 'node:dgram';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtempSync, writeFileSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
const script = fileURLToPath(new URL('../../../scripts/collect-training-data.py', import.meta.url));
async function record(dir, port, sequence) {
  const child = spawn(python, ['-u', script, '--port', String(port), '--label', 'walking', '--duration', '0.02', '--output-dir', dir], { env: { ...process.env, PYTHONPATH: dir } });
  const exited = once(child, 'exit'); let timer;
  const tx = dgram.createSocket('udp4');
  try {
    await new Promise((resolve, reject) => {
      let text = ''; timer = setTimeout(() => reject(new Error('collector startup timeout')), 2000);
      child.stderr.on('data', b => { text += b; if (text.includes("Collecting 'walking'")) { clearTimeout(timer); resolve(); } });
      child.once('error', reject);
    });
    const packet = Buffer.alloc(24); packet.writeUInt32LE(0xC5110001); packet[4]=1; packet[5]=1; packet.writeUInt16LE(2,6); packet.writeUInt32LE(2437,8); packet.writeUInt32LE(sequence,12); packet.writeInt8(-50,16); Buffer.from([3,4,6,8]).copy(packet,20);
    await new Promise((resolve,reject)=>tx.send(packet,port,'127.0.0.1',e=>e?reject(e):resolve()));
    const [code] = await exited; assert.equal(code,0);
  } finally { clearTimeout(timer); tx.close(); if(child.exitCode===null){child.kill('SIGINT');await exited;} }
}
test('public collector CLI preserves distinct same-second sessions and matching metadata', async () => {
  const dir=mkdtempSync(join(tmpdir(),'ruview-session-collision-'));
  const probe=dgram.createSocket('udp4'); probe.bind(0,'127.0.0.1');await once(probe,'listening');const port=probe.address().port;await new Promise(resolve=>probe.close(resolve));
  try {
    writeFileSync(join(dir,'sitecustomize.py'), 'import datetime\nclass Fixed(datetime.datetime):\n @classmethod\n def now(cls, tz=None):\n  return cls(2026,10,11,12,0,0,tzinfo=tz)\ndatetime.datetime=Fixed\n');
    await record(dir,port,1); await record(dir,port,2);
    const metadata=readdirSync(dir).filter(f=>f.endsWith('.csi.meta.json')).map(f=>JSON.parse(readFileSync(join(dir,f),'utf8')));
    assert.equal(metadata.length,2);assert.equal(new Set(metadata.map(m=>m.id)).size,2);
    const sequences=[];
    for(const m of metadata){const frames=readFileSync(m.file_path,'utf8').trim().split('\n').map(JSON.parse);assert.equal(m.frame_count,frames.length);assert.equal(frames.length,1);sequences.push(frames[0].features.seq);}
    assert.deepEqual(sequences.sort(),[1,2]);
    const manifest=JSON.parse(readFileSync(join(dir,'manifest.json'),'utf8'));assert.equal(manifest.num_sessions,2);assert.equal(manifest.total_frames,2);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
