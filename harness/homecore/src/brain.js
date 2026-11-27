// SPDX-License-Identifier: MIT
// Reviewable shared Homecore knowledge. Private indexes stay outside the package.

import { createHash } from 'node:crypto';
import { existsSync, readFileSync, realpathSync, statSync } from 'node:fs';
import { dirname, isAbsolute, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
export const CORPUS_PATH = join(ROOT, 'brain', 'corpus', 'core.jsonl');
const SECRET = /(-----BEGIN [A-Z ]*PRIVATE KEY-----|(?:api[_-]?key|token|password|secret|setup[_-]?code|pairing)\s*[:=]\s*\S+)/i;
const INJECTION = /\b(ignore (?:all|the|previous)|system prompt|developer message|execute this|run this command)\b/i;
const EVIDENCE = new Set(['REPOSITORY', 'POLICY', 'ADR', 'MEASURED', 'SYNTHETIC']);

function sha256(text) {
  return createHash('sha256').update(text).digest('hex');
}

export function validateBrainRecord(record, { canonical = false } = {}) {
  const errors = [];
  if (!record || typeof record !== 'object' || Array.isArray(record)) {
    return ['record must be an object'];
  }
  for (const key of ['id', 'title', 'content', 'evidence']) {
    if (typeof record[key] !== 'string' || !record[key].trim()) {
      errors.push(`${key} must be a non-empty string`);
    }
  }
  if (!/^[a-z0-9][a-z0-9-]{2,63}$/.test(record.id || '')) {
    errors.push('id must be a lowercase slug');
  }
  if (!EVIDENCE.has(record.evidence)) errors.push(`unsupported evidence: ${record.evidence}`);
  if (
    !record.source
    || typeof record.source.path !== 'string'
    || !Number.isInteger(record.source.line)
    || record.source.line < 1
  ) {
    errors.push('source.path and positive source.line are required');
  } else if (
    isAbsolute(record.source.path)
    || record.source.path.split(/[\\/]/).includes('..')
    || /^[A-Za-z]:/.test(record.source.path)
  ) {
    errors.push('source.path must be repository-relative without traversal');
  }
  if (typeof record.source?.path === 'string'
    && !record.source.path.split(/[\\/]/).some(part => part.trim() && part !== '.')) {
    errors.push('source.path must identify a repository-relative file');
  }
  if (canonical || record.source?.endLine !== undefined || record.source?.digest !== undefined) {
    if (
      !Number.isInteger(record.source?.endLine)
      || record.source.endLine < record.source.line
      || record.source.endLine - record.source.line > 63
    ) {
      errors.push('source.endLine must bound a source span of at most 64 lines');
    }
    if (!/^[a-f0-9]{64}$/.test(record.source?.digest || '')) {
      errors.push('source.digest must be a lowercase SHA-256 digest');
    }
  }
  if (!Array.isArray(record.tags) || record.tags.some((tag) => typeof tag !== 'string')) {
    errors.push('tags must be strings');
  }
  if ((record.content || '').length > 8192) errors.push('content exceeds 8192 characters');
  if ((record.title || '').length > 200) errors.push('title exceeds 200 characters');
  if (canonical && record.reviewed !== true) errors.push('canonical records must be reviewed');
  const combined = `${record.title || ''}\n${record.content || ''}`;
  if (SECRET.test(combined)) errors.push('record appears to contain a secret');
  if (INJECTION.test(combined)) errors.push('record contains instruction-like prompt injection');
  return errors;
}

export function loadBrain(path = CORPUS_PATH) {
  const raw = readFileSync(path, 'utf8').replace(/\r\n/g, '\n');
  if (Buffer.byteLength(raw) > 1_048_576) throw new Error('brain corpus exceeds 1 MiB');
  const records = raw.split('\n').filter(Boolean).map((line, index) => {
    if (Buffer.byteLength(line) > 16_384) {
      throw new Error(`brain line ${index + 1}: exceeds 16 KiB`);
    }
    let record;
    try {
      record = JSON.parse(line);
    } catch (error) {
      throw new Error(`brain line ${index + 1}: ${error.message}`);
    }
    const errors = validateBrainRecord(record, { canonical: true });
    if (errors.length) throw new Error(`brain line ${index + 1}: ${errors.join('; ')}`);
    return Object.freeze(record);
  });
  if (records.length > 1000) throw new Error('brain corpus exceeds 1000 records');
  const ids = new Set();
  for (const record of records) {
    if (ids.has(record.id)) throw new Error(`duplicate brain id: ${record.id}`);
    ids.add(record.id);
  }
  return { records, digest: sha256(raw), bytes: Buffer.byteLength(raw) };
}

function terms(value) {
  return new Set(String(value).toLowerCase().match(/[a-z0-9][a-z0-9_-]{1,}/g) || []);
}

export function searchBrain(query, { limit = 8, path = CORPUS_PATH } = {}) {
  const wanted = terms(query);
  if (!wanted.size) return [];
  const { records, digest } = loadBrain(path);
  return records.map((record) => {
    const title = terms(record.title);
    const body = terms(record.content);
    const tags = new Set(record.tags.map((tag) => tag.toLowerCase()));
    let score = 0;
    for (const term of wanted) {
      score += title.has(term) ? 5 : tags.has(term) ? 3 : body.has(term) ? 1 : 0;
    }
    return {
      ...record,
      score,
      citation: record.source.endLine === record.source.line
        ? `${record.source.path}:${record.source.line}`
        : `${record.source.path}:${record.source.line}-${record.source.endLine}`,
      corpusDigest: digest,
    };
  })
    .filter((record) => record.score > 0)
    .sort((a, b) => b.score - a.score || a.id.localeCompare(b.id))
    .slice(0, Math.max(1, Math.min(Number(limit) || 8, 25)));
}

export function verifyBrain({ repo = process.cwd(), path = CORPUS_PATH } = {}) {
  const root = resolve(repo);
  const realRoot = realpathSync(root);
  const { records, digest, bytes } = loadBrain(path);
  const findings = [];
  for (const record of records) {
    const source = resolve(root, record.source.path);
    const rel = relative(root, source);
    if (isAbsolute(rel) || rel.startsWith('..') || !existsSync(source)) {
      findings.push({ id: record.id, reason: 'source_missing', source: record.source.path });
      continue;
    }
    try {
      const real = realpathSync(source);
      const realRel = relative(realRoot, real);
      if (isAbsolute(realRel) || realRel.startsWith('..')) {
        findings.push({ id: record.id, reason: 'source_escape', source: record.source.path });
        continue;
      }
      if (!statSync(real).isFile()) {
        findings.push({ id: record.id, reason: 'source_not_file', source: record.source.path });
        continue;
      }
      const sourceLines = readFileSync(real, 'utf8').split(/\r?\n/);
      if (record.source.endLine > sourceLines.length) {
        findings.push({
          id: record.id,
          reason: 'source_line_missing',
          source: record.source.path,
          line: record.source.endLine,
        });
        continue;
      }
      const sourceSpan = sourceLines
        .slice(record.source.line - 1, record.source.endLine)
        .join('\n');
      if (sha256(sourceSpan) !== record.source.digest) {
        findings.push({
          id: record.id,
          reason: 'source_digest_mismatch',
          source: record.source.path,
          line: record.source.line,
          endLine: record.source.endLine,
        });
      }
    } catch (error) {
      findings.push({ id: record.id, reason: 'source_unreadable', source: record.source.path, code: error.code });
    }
  }
  return { ok: findings.length === 0, records: records.length, digest, bytes, findings };
}

export function makeProposal(input) {
  const record = {
    id: String(input.id || '').trim(),
    title: String(input.title || '').trim(),
    content: String(input.content || '').trim(),
    source: {
      path: String(input.sourcePath || '').trim(),
      line: Number(input.sourceLine),
    },
    evidence: String(input.evidence || 'REPOSITORY').toUpperCase(),
    tags: String(input.tags || '').split(',').map((tag) => tag.trim()).filter(Boolean),
    contributor: String(input.contributor || '').trim() || 'unknown',
    reviewed: false,
  };
  const errors = validateBrainRecord(record);
  return errors.length
    ? { ok: false, errors }
    : { ok: true, proposal: record, jsonl: JSON.stringify(record) };
}
