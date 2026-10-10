import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';

test('MCP drops an oversized unfinished line before newline and accepts the next fragmented request', async () => {
  const child = spawn(process.execPath, ['bin/cli.js', 'mcp', 'start'], {stdio:['pipe','pipe','pipe']});
  let stderr = '', stdout = '';
  child.stderr.on('data', data => stderr += data);
  child.stdout.on('data', data => stdout += data);
  try {
    const chunk = Buffer.alloc(64 * 1024, 0x78);
    for(let i=0;i<6;i++) if(!child.stdin.write(chunk)) await once(child.stdin, 'drain');
    const deadline = Date.now()+1000;
    while(!stderr.includes('oversized JSON-RPC line dropped') && Date.now()<deadline) await new Promise(resolve => setTimeout(resolve, 10));
    assert.match(stderr, /oversized JSON-RPC line dropped/, 'discard the unfinished line as soon as the bound is exceeded');
    child.stdin.write('\n');
    const request = Buffer.from(JSON.stringify({jsonrpc:'2.0',id:'unicode-✓',method:'ping'})+'\n');
    const split = request.indexOf(Buffer.from('✓'))+1;
    child.stdin.write(request.subarray(0, split));
    child.stdin.end(request.subarray(split));
    const code = await Promise.race([once(child,'close').then(([code])=>code),new Promise((_,reject)=>setTimeout(()=>reject(new Error('server did not finish')),3000))]);
    assert.equal(code,0);
    assert.deepEqual(JSON.parse(stdout.trim()), {jsonrpc:'2.0',id:'unicode-✓',result:{}});
  } finally { child.kill('SIGKILL'); }
});
