// Software-only execution of the real Python pump; no serial device is opened.
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { PUMP_SCRIPT } from '../src/devices/serial-pump.js';

const python = ['python3', 'python'].find(p => spawnSync(p, ['--version'], { stdio: 'ignore' }).status === 0);
const fakeSerial = `import os,sys

def record(value):
 with open(os.environ['PUMP_TRACE'], 'a') as out: out.write(value+'\\n')
class BrokenOutput:
 def write(self,value): raise RuntimeError('injected output failure')
 def flush(self): pass
class Serial:
 def __init__(self,*args,**kwargs):
  record('open')
  if os.environ['PUMP_FAILURE']=='open': raise RuntimeError('injected open failure')
 @property
 def dtr(self): return False
 @dtr.setter
 def dtr(self,value):
  record('dtr')
  failure=os.environ['PUMP_FAILURE']
  if failure=='dtr-output': sys.stdout=BrokenOutput()
  if failure=='dtr-interrupt': raise KeyboardInterrupt('injected DTR interrupt')
  if failure in ('dtr', 'dtr-output'): raise RuntimeError('injected unsupported DTR')
 def write(self,data):
  value=data.hex(); record('write:'+value)
  failure=os.environ['PUMP_FAILURE']
  if failure==value or (failure=='read-stop' and value=='a525'): raise RuntimeError('injected write failure')
 def flush(self):
  record('flush')
  if os.environ['PUMP_FAILURE']=='flush': raise RuntimeError('injected flush failure')
 def read(self,size):
  record('read')
  if os.environ['PUMP_FAILURE']=='output':
   sys.stdout=BrokenOutput()
   return b'abc'
  raise RuntimeError('injected read failure')
 def close(self): record('close')
`;

for (const [name, failure, seconds, stop, expected, successful, start = 'a520', dtr = 'keep'] of [
  ['normal completion', '', '0', 'a525', ['open', 'write:a520', 'flush', 'write:a525', 'flush', 'close'], true],
  ['read failure', '', '1', 'a525', ['open', 'write:a520', 'flush', 'read', 'write:a525', 'flush', 'close'], false],
  ['start failure', 'a520', '0', 'a525', ['open', 'write:a520', 'write:a525', 'flush', 'close'], false],
  ['stop failure', 'a525', '0', 'a525', ['open', 'write:a520', 'flush', 'write:a525', 'close'], false],
  ['output failure', 'output', '1', 'a525', ['open', 'write:a520', 'flush', 'read', 'write:a525', 'flush', 'close'], false],
  ['open failure', 'open', '0', 'a525', ['open'], false],
  ['start flush failure', 'flush', '0', 'a525', ['open', 'write:a520', 'flush', 'write:a525', 'flush', 'close'], false],
  ['read and stop failures', 'read-stop', '1', 'a525', ['open', 'write:a520', 'flush', 'read', 'write:a525', 'close'], false],
  ['unsupported DTR', 'dtr', '0', 'a525', ['open', 'dtr', 'write:a520', 'flush', 'write:a525', 'flush', 'close'], true, 'a520', 'low'],
  ['unsupported DTR reporting failure', 'dtr-output', '0', 'a525', ['open', 'dtr', 'write:a525', 'flush', 'close'], false, 'a520', 'low'],
  ['DTR interrupt', 'dtr-interrupt', '0', 'a525', ['open', 'dtr', 'write:a525', 'flush', 'close'], false, 'a520', 'low'],
  ['passive completion', '', '0', '', ['open', 'close'], true, ''],
  ['passive read failure', '', '1', '', ['open', 'read', 'close'], false, ''],
]) {
  test(`serial pump cleanup on ${name}`, { skip: !python && 'Python is not installed' }, () => {
    const dir = mkdtempSync(join(tmpdir(), 'ruview-pump-cleanup-'));
    try {
      const trace = join(dir, 'trace.txt');
      writeFileSync(join(dir, 'serial.py'), fakeSerial);
      const result = spawnSync(python, ['-c', PUMP_SCRIPT, 'synthetic-port', '115200', seconds, '4096', start, stop, dtr], {
        cwd: dir, encoding: 'utf8', timeout: 5000,
        env: { PATH: process.env.PATH, SYSTEMROOT: process.env.SYSTEMROOT || '', PYTHONPATH: dir, PUMP_TRACE: trace, PUMP_FAILURE: failure },
      });
      assert.ifError(result.error);
      assert.equal(result.status === 0, successful, result.stderr);
      assert.deepEqual(readFileSync(trace, 'utf8').trim().split(/\r?\n/), expected);
      if (failure === 'read-stop') {
        assert.match(result.stderr, /injected read failure/);
        assert.match(result.stderr, /injected write failure/);
      }
      if (failure === 'dtr') assert.match(result.stdout, /DTR_UNSUPPORTED/);
      if (failure === 'open') assert.match(result.stdout, /OPEN_ERROR/);
      assert.equal(result.stdout.includes('END 0'), successful, 'only completed captures report END');
    } finally { rmSync(dir, { recursive: true, force: true }); }
  });
}
