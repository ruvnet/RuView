// SPDX-License-Identifier: MIT

import { lstatSync, readFileSync, readdirSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';

function walk(path, output) {
  const stat = lstatSync(path);
  if (stat.isSymbolicLink()) throw new TypeError(`Symbolic links are prohibited in the package surface: ${path}`);
  if (stat.isDirectory()) {
    for (const entry of readdirSync(path).sort()) walk(join(path, entry), output);
    return;
  }
  if (!stat.isFile()) throw new TypeError(`Non-file package entry is prohibited: ${path}`);
  output.push(path);
}

export function listPackageSurface(root) {
  const packageRoot = resolve(root);
  const pkg = JSON.parse(readFileSync(join(packageRoot, 'package.json'), 'utf8'));
  if (!Array.isArray(pkg.files) || pkg.files.length === 0) throw new TypeError('package.json files must be a non-empty array');
  const paths = [];
  for (const entry of pkg.files) {
    if (typeof entry !== 'string' || entry.length === 0 || /[*?{}[\]]/u.test(entry)) {
      throw new TypeError('Package file entries must be explicit paths without globs');
    }
    const target = resolve(packageRoot, entry);
    const rel = relative(packageRoot, target);
    if (rel.startsWith('..') || rel === '') throw new TypeError(`Package entry escapes or names the package root: ${entry}`);
    walk(target, paths);
  }
  paths.push(join(packageRoot, 'package.json'));
  return [...new Set(paths)].sort();
}

export function packageRelativeNames(root) {
  const packageRoot = resolve(root);
  return listPackageSurface(packageRoot).map((path) => relative(packageRoot, path).replaceAll('\\', '/'));
}

export function compareRecordedSurface(recorded, packaged) {
  const selfFiles = new Set(['.harness/manifest.json', '.harness/manifest.sha256']);
  const expected = new Set(recorded);
  const actual = new Set(packaged.filter((name) => !selfFiles.has(name)));
  return [
    ...[...actual].filter((name) => !expected.has(name)).sort().map((name) => `${name}:unmanifested-packaged-file`),
    ...[...expected].filter((name) => !actual.has(name)).sort().map((name) => `${name}:manifested-file-not-packaged`),
  ];
}
