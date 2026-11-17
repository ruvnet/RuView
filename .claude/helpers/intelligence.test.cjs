'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const helper = path.join(__dirname, 'intelligence.cjs');

function fixture(t) {
  const tempRoot = fs.realpathSync(os.tmpdir());
  const root = fs.mkdtempSync(path.join(tempRoot, 'ruview-memory-'));
  t.after(() => {
    const resolved = fs.realpathSync(root);
    assert.equal(path.dirname(resolved), tempRoot);
    assert.ok(path.basename(resolved).startsWith('ruview-memory-'));
    fs.rmSync(resolved, { recursive: true, force: true });
  });
  const repo = path.join(root, 'repo');
  const home = path.join(root, 'home');
  fs.mkdirSync(repo);
  fs.mkdirSync(home);
  function write(file, content) {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, content);
    return file;
  }
  function run(expression = 'i.init()') {
    const script = `const os=require('node:os'); os.homedir=()=>${JSON.stringify(home)}; ` +
      `const i=require(${JSON.stringify(helper)}); console.log(JSON.stringify(${expression}));`;
    const child = spawnSync(process.execPath, ['-e', script], { cwd: repo, encoding: 'utf8' });
    assert.equal(child.status, 0, child.stderr);
    return JSON.parse(child.stdout);
  }
  const data = path.join(repo, '.claude-flow', 'data');
  const read = name => JSON.parse(fs.readFileSync(path.join(data, name), 'utf8'));
  return { root, repo, home, data, write, run, read };
}

test('bootstrap indexes only checkout-local memory; global projects stay private', t => {
  const f = fixture(t);
  f.write(path.join(f.home, '.claude', 'projects', 'unrelated', 'memory', 'MEMORY.md'),
    '# PRIVATE PROJECT\nprivate unrelated project memory must never be imported');
  f.write(path.join(f.repo, '.claude', 'memory', 'MEMORY.md'),
    '# Local sensing\ncheckout local sensing memory is available');
  f.write(path.join(f.repo, '.claude-flow', 'memory', 'notes.md'),
    '# Local router\ncheckout local router memory is available');
  assert.equal(f.run().nodes, 2);
  for (const name of ['auto-memory-store.json', 'ranked-context.json', 'graph-state.json']) {
    const text = JSON.stringify(f.read(name));
    assert.ok(!text.includes('PRIVATE PROJECT') && !text.includes('unrelated'), name);
  }
  assert.match(f.run('i.getContext("local sensing")'), /Local sensing/);
  assert.match(f.run('(i.consolidate(), i.getContext("local sensing"))'), /Local sensing/,
    'consolidation must preserve the scoped context cache');
});

test('a checkout without local memory imports nothing from the user home', t => {
  const f = fixture(t);
  f.write(path.join(f.home, '.claude', 'projects', 'other', 'memory', 'MEMORY.md'),
    '# Private\nprivate project information outside this checkout');
  assert.equal(f.run().nodes, 0);
  assert.equal(fs.existsSync(path.join(f.data, 'auto-memory-store.json')), false);
});

test('legacy cross-project entries are purged even with a fresh graph cache', t => {
  const f = fixture(t);
  const sourceFile = f.write(path.join(f.home, '.claude', 'projects', 'other', 'memory', 'MEMORY.md'),
    '# Private\nprivate information from a different project');
  f.write(path.join(f.data, 'auto-memory-store.json'), JSON.stringify([
    { id: 'private', summary: 'Private', content: 'private information', metadata: { bootstrapped: true, sourceFile } },
  ]));
  f.write(path.join(f.data, 'graph-state.json'), JSON.stringify({ nodeCount: 1, updatedAt: Date.now() }));
  f.write(path.join(f.data, 'ranked-context.json'), JSON.stringify({ version: 1,
    entries: [{ id: 'private', summary: 'Private', words: ['private'], pageRank: 1 }] }));
  assert.equal(f.run('i.getContext("private")'), null, 'legacy cache must fail closed before init');
  assert.equal(f.run().nodes, 0);
  assert.deepEqual(f.read('auto-memory-store.json'), []);
  assert.deepEqual(f.read('ranked-context.json').entries, []);
  assert.deepEqual(f.read('graph-state.json').nodes, {});
  assert.equal(f.run('i.getContext("private")'), null);
});

test('a memory directory junction outside the checkout is not followed', t => {
  const f = fixture(t);
  const outside = path.join(f.root, 'repo-neighbor', 'memory');
  f.write(path.join(outside, 'MEMORY.md'), '# Private\nprivate neighboring project information');
  fs.mkdirSync(path.join(f.repo, '.claude'), { recursive: true });
  fs.symlinkSync(outside, path.join(f.repo, '.claude', 'memory'), process.platform === 'win32' ? 'junction' : 'dir');
  assert.equal(f.run().nodes, 0);
});

test('a markdown symlink outside the checkout is not followed', t => {
  const f = fixture(t);
  const outside = f.write(path.join(f.root, 'private.md'), '# Private\nprivate file outside checkout boundary');
  const dir = path.join(f.repo, '.claude', 'memory');
  fs.mkdirSync(dir, { recursive: true });
  try { fs.symlinkSync(outside, path.join(dir, 'MEMORY.md')); }
  catch (error) {
    if (process.platform === 'win32' && error.code === 'EPERM') { t.skip('Windows file symlinks require Developer Mode'); return; }
    throw error;
  }
  assert.equal(f.run().nodes, 0);
});
