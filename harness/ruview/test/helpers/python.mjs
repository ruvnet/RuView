import { spawnSync } from 'node:child_process';

// Windows commonly installs python.exe without a python3.exe alias.
const candidates = process.platform === 'win32' ? ['python', 'python3'] : ['python3', 'python'];
export const python = candidates.find(command => {
  const result = spawnSync(command, ['-c', 'import sys; assert sys.version_info >= (3, 9)'], {
    stdio: 'ignore', timeout: 5000, windowsHide: true,
  });
  return result.status === 0;
});
if (!python) throw new Error('Capture regression tests require Python 3.9 or newer on PATH');
