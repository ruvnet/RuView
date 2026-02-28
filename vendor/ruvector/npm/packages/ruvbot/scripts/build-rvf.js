#!/usr/bin/env node
/**
 * build-rvf.js — Assemble RuvBot as a self-contained .rvf file.
 *
 * The output contains:
 *   KERNEL_SEG  (0x0E) — Real Linux 6.6 microkernel (bzImage, x86_64)
 *   WASM_SEG    (0x10) — RuvBot runtime bundle (Node.js application)
 *   META_SEG    (0x07) — Package metadata (name, version, config)
 *   PROFILE_SEG (0x0B) — AI assistant domain profile
 *   WITNESS_SEG (0x0A) — Build provenance chain
 *   MANIFEST_SEG(0x05) — Segment directory + epoch
 *
 * Usage:
 *   node scripts/build-rvf.js --kernel /path/to/bzImage [--output ruvbot.rvf]
 *
 * The --kernel flag provides a real Linux bzImage to embed. If omitted,
 * the script looks for kernel/bzImage relative to the package root.
 */

'use strict';

const { writeFileSync, readFileSync, existsSync, readdirSync, statSync, mkdirSync } = require('fs');
const { join, resolve, isAbsolute } = require('path');
const { createHash } = require('crypto');
const { execSync } = require('child_process');
const { gzipSync } = require('zlib');

// ─── RVF format constants ───────────────────────────────────────────────────
const SEGMENT_MAGIC   = 0x5256_4653;  // "RVFS" big-endian
const SEGMENT_VERSION = 1;
const KERNEL_MAGIC    = 0x5256_4B4E;  // "RVKN" big-endian
const WASM_MAGIC      = 0x5256_574D;  // "RVWM" big-endian

// Segment type discriminators
const SEG_MANIFEST  = 0x05;
const SEG_META      = 0x07;
const SEG_WITNESS   = 0x0A;
const SEG_PROFILE   = 0x0B;
const SEG_KERNEL    = 0x0E;
const SEG_WASM      = 0x10;

// Kernel constants
const KERNEL_ARCH_X86_64     = 0x00;
const KERNEL_TYPE_MICROLINUX = 0x01;
const KERNEL_FLAG_HAS_NETWORKING = 1 << 3;
const KERNEL_FLAG_HAS_QUERY_API  = 1 << 4;
const KERNEL_FLAG_HAS_ADMIN_API  = 1 << 6;
const KERNEL_FLAG_RELOCATABLE    = 1 << 11;
const KERNEL_FLAG_HAS_VIRTIO_NET = 1 << 12;
const KERNEL_FLAG_HAS_VIRTIO_BLK = 1 << 13;
const KERNEL_FLAG_HAS_VSOCK      = 1 << 14;
const KERNEL_FLAG_COMPRESSED     = 1 << 10;

// WASM constants
const WASM_ROLE_COMBINED = 0x02;
const WASM_TARGET_NODE   = 0x01;

// ─── Binary helpers ─────────────────────────────────────────────────────────

function writeU8(buf, offset, val) {
  buf[offset] = val & 0xFF;
  return offset + 1;
}

function writeU16LE(buf, offset, val) {
  buf.writeUInt16LE(val, offset);
  return offset + 2;
}

function writeU32LE(buf, offset, val) {
  buf.writeUInt32LE(val >>> 0, offset);
  return offset + 4;
}

function writeU64LE(buf, offset, val) {
  const big = BigInt(Math.floor(val));
  buf.writeBigUInt64LE(big, offset);
  return offset + 8;
}

function contentHash16(payload) {
  return createHash('sha256').update(payload).digest().subarray(0, 16);
}

// ─── Segment header writer (64 bytes) ───────────────────────────────────────

function makeSegmentHeader(segType, segId, payloadLength, payload) {
  const buf = Buffer.alloc(64);
  writeU32LE(buf, 0x00, SEGMENT_MAGIC);
  writeU8(buf, 0x04, SEGMENT_VERSION);
  writeU8(buf, 0x05, segType);
  writeU16LE(buf, 0x06, 0);                       // flags
  writeU64LE(buf, 0x08, segId);
  writeU64LE(buf, 0x10, payloadLength);
  writeU64LE(buf, 0x18, Date.now() * 1e6);        // timestamp_ns
  writeU8(buf, 0x20, 0);                           // checksum_algo (CRC32C)
  writeU8(buf, 0x21, 0);                           // compression
  writeU16LE(buf, 0x22, 0);                        // reserved_0
  writeU32LE(buf, 0x24, 0);                        // reserved_1
  contentHash16(payload).copy(buf, 0x28, 0, 16);   // content_hash
  writeU32LE(buf, 0x38, 0);                        // uncompressed_len
  writeU32LE(buf, 0x3C, 0);                        // alignment_pad
  return buf;
}

// ─── Kernel header (128 bytes) ──────────────────────────────────────────────

function makeKernelHeader(imageSize, compressedSize, cmdlineLen, isCompressed) {
  const buf = Buffer.alloc(128);
  writeU32LE(buf, 0x00, KERNEL_MAGIC);
  writeU16LE(buf, 0x04, 1);                        // header_version
  writeU8(buf, 0x06, KERNEL_ARCH_X86_64);
  writeU8(buf, 0x07, KERNEL_TYPE_MICROLINUX);
  const flags = KERNEL_FLAG_HAS_NETWORKING
              | KERNEL_FLAG_HAS_QUERY_API
              | KERNEL_FLAG_HAS_ADMIN_API
              | KERNEL_FLAG_RELOCATABLE
              | KERNEL_FLAG_HAS_VIRTIO_NET
              | KERNEL_FLAG_HAS_VIRTIO_BLK
              | (isCompressed ? KERNEL_FLAG_COMPRESSED : 0);
  writeU32LE(buf, 0x08, flags);
  writeU32LE(buf, 0x0C, 64);                       // min_memory_mb
  writeU64LE(buf, 0x10, 0x1000000);                // entry_point (16 MB default load)
  writeU64LE(buf, 0x18, imageSize);                // image_size (uncompressed)
  writeU64LE(buf, 0x20, compressedSize);           // compressed_size
  writeU8(buf, 0x28, isCompressed ? 1 : 0);       // compression (0=none, 1=gzip)
  writeU8(buf, 0x29, 0x00);                        // api_transport (TcpHttp)
  writeU16LE(buf, 0x2A, 3000);                     // api_port
  writeU16LE(buf, 0x2C, 1);                        // api_version
  // 0x2E: 2 bytes padding
  // 0x30: image_hash (32 bytes) — filled by caller
  // 0x50: build_id (16 bytes)
  writeU64LE(buf, 0x60, Math.floor(Date.now() / 1000)); // build_timestamp
  writeU8(buf, 0x68, 1);                           // vcpu_count
  // 0x69: reserved_0
  // 0x6A: 2 bytes padding
  writeU32LE(buf, 0x6C, 128);                      // cmdline_offset
  writeU32LE(buf, 0x70, cmdlineLen);               // cmdline_length
  return buf;
}

// ─── WASM header (64 bytes) ─────────────────────────────────────────────────

function makeWasmHeader(bytecodeSize) {
  const buf = Buffer.alloc(64);
  writeU32LE(buf, 0x00, WASM_MAGIC);
  writeU16LE(buf, 0x04, 1);                        // header_version
  writeU8(buf, 0x06, WASM_ROLE_COMBINED);          // role
  writeU8(buf, 0x07, WASM_TARGET_NODE);            // target
  writeU16LE(buf, 0x08, 0);                        // required_features
  writeU16LE(buf, 0x0A, 12);                       // export_count
  writeU32LE(buf, 0x0C, bytecodeSize);             // bytecode_size
  writeU32LE(buf, 0x10, 0);                        // compressed_size
  writeU8(buf, 0x14, 0);                           // compression
  writeU8(buf, 0x15, 2);                           // min_memory_pages
  writeU16LE(buf, 0x16, 0);                        // max_memory_pages
  writeU16LE(buf, 0x18, 0);                        // table_count
  // 0x1A: 2 bytes padding
  // 0x1C: bytecode_hash (32 bytes) — filled by caller
  writeU8(buf, 0x3C, 0);                           // bootstrap_priority
  writeU8(buf, 0x3D, 0);                           // interpreter_type
  return buf;
}

// ─── Load real kernel image ─────────────────────────────────────────────────

function loadKernelImage(kernelPath) {
  if (!existsSync(kernelPath)) {
    console.error(`ERROR: Kernel image not found: ${kernelPath}`);
    console.error('Build one with: cd /tmp/linux-6.6.80 && make tinyconfig && make -j$(nproc) bzImage');
    process.exit(1);
  }

  const raw = readFileSync(kernelPath);
  const stat = statSync(kernelPath);
  console.log(`    Loaded: ${kernelPath} (${(raw.length / 1024).toFixed(0)} KB)`);

  // Verify it looks like a real kernel (ELF or bzImage magic)
  const magic = raw.readUInt16LE(0);
  const elfMagic = raw.subarray(0, 4);
  if (elfMagic[0] === 0x7F && elfMagic[1] === 0x45 && elfMagic[2] === 0x4C && elfMagic[3] === 0x46) {
    console.log('    Format: ELF vmlinux');
  } else if (raw.length > 0x202 && raw.readUInt16LE(0x1FE) === 0xAA55) {
    console.log('    Format: bzImage (bootable)');
  } else {
    console.log('    Format: raw kernel image');
  }

  // Gzip compress for smaller RVF
  const compressed = gzipSync(raw, { level: 9 });
  const ratio = ((1 - compressed.length / raw.length) * 100).toFixed(1);
  console.log(`    Compressed: ${(compressed.length / 1024).toFixed(0)} KB (${ratio}% reduction)`);

  return { raw, compressed };
}

// ─── Build the runtime bundle ───────────────────────────────────────────────

function buildRuntimeBundle(pkgDir) {
  const distDir = join(pkgDir, 'dist');
  const binDir = join(pkgDir, 'bin');
  const files = [];

  if (existsSync(distDir)) collectFiles(distDir, '', files);
  if (existsSync(binDir)) collectFiles(binDir, 'bin/', files);

  const pkgJsonPath = join(pkgDir, 'package.json');
  if (existsSync(pkgJsonPath)) {
    files.push({ path: 'package.json', data: readFileSync(pkgJsonPath) });
  }

  // Bundle format: [file_count(u32)] [file_table] [file_data]
  const fileCount = Buffer.alloc(4);
  fileCount.writeUInt32LE(files.length, 0);

  let tableSize = 0;
  for (const f of files) {
    tableSize += 2 + 8 + 8 + Buffer.byteLength(f.path, 'utf8');
  }

  let dataOffset = 4 + tableSize;
  const tableEntries = [];
  for (const f of files) {
    const pathBuf = Buffer.from(f.path, 'utf8');
    const entry = Buffer.alloc(2 + 8 + 8 + pathBuf.length);
    let o = writeU16LE(entry, 0, pathBuf.length);
    o = writeU64LE(entry, o, dataOffset);
    o = writeU64LE(entry, o, f.data.length);
    pathBuf.copy(entry, o);
    tableEntries.push(entry);
    dataOffset += f.data.length;
  }

  return Buffer.concat([fileCount, ...tableEntries, ...files.map(f => f.data)]);
}

function collectFiles(dir, prefix, files) {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    const rel = prefix + name;
    const stat = statSync(full);
    if (stat.isDirectory()) collectFiles(full, rel + '/', files);
    else if (stat.isFile()) files.push({ path: rel, data: readFileSync(full) });
  }
}

// ─── Build META_SEG ─────────────────────────────────────────────────────────

function buildMetaPayload(pkgDir, kernelInfo) {
  const pkgJson = JSON.parse(readFileSync(join(pkgDir, 'package.json'), 'utf8'));
  return Buffer.from(JSON.stringify({
    name: pkgJson.name,
    version: pkgJson.version,
    description: pkgJson.description,
    format: 'rvf-self-contained',
    runtime: 'node',
    runtime_version: '>=18.0.0',
    arch: 'x86_64',
    kernel: {
      type: 'microlinux',
      version: '6.6.80',
      config: 'tinyconfig+virtio+net',
      image_size: kernelInfo.rawSize,
      compressed_size: kernelInfo.compressedSize,
    },
    build_time: new Date().toISOString(),
    builder: 'ruvbot/build-rvf.js',
    capabilities: [
      'self-booting',
      'api-server',
      'chat',
      'vector-search',
      'self-learning',
      'multi-llm',
      'security-scanning',
    ],
    dependencies: Object.keys(pkgJson.dependencies || {}),
    entrypoint: 'bin/ruvbot.js',
    api_port: 3000,
    firecracker_compatible: true,
  }), 'utf8');
}

// ─── Build PROFILE_SEG ──────────────────────────────────────────────────────

function buildProfilePayload() {
  return Buffer.from(JSON.stringify({
    profile_id: 0x42,
    domain: 'ai-assistant',
    name: 'RuvBot',
    version: '0.2.0',
    capabilities: {
      chat: true,
      vector_search: true,
      embeddings: true,
      self_learning: true,
      multi_model: true,
      security: true,
      self_booting: true,
    },
    models: [
      'claude-sonnet-4-20250514',
      'gemini-2.0-flash',
      'gpt-4o',
      'openrouter/*',
    ],
    boot_config: {
      vcpus: 1,
      memory_mb: 64,
      api_port: 3000,
      cmdline: 'console=ttyS0 ruvbot.mode=rvf',
    },
  }), 'utf8');
}

// ─── Build WITNESS_SEG ──────────────────────────────────────────────────────

function buildWitnessPayload(kernelHash, runtimeHash) {
  return Buffer.from(JSON.stringify({
    witness_type: 'build_provenance',
    timestamp: new Date().toISOString(),
    builder: {
      tool: 'build-rvf.js',
      node_version: process.version,
      platform: process.platform,
      arch: process.arch,
    },
    artifacts: {
      kernel: { hash_sha256: kernelHash, type: 'linux-6.6-bzimage' },
      runtime: { hash_sha256: runtimeHash, type: 'nodejs-bundle' },
    },
    chain: [],
  }), 'utf8');
}

// ─── Assemble the RVF ───────────────────────────────────────────────────────

function assembleRvf(pkgDir, outputPath, kernelPath) {
  console.log('Building self-contained RuvBot RVF...');
  console.log(`  Package: ${pkgDir}`);
  console.log(`  Kernel:  ${kernelPath}`);
  console.log(`  Output:  ${outputPath}\n`);

  let segId = 1;
  const segments = [];
  const segDir = [];

  // 1. KERNEL_SEG — Real Linux microkernel
  console.log('  [1/6] Embedding Linux 6.6 microkernel...');
  const { raw: kernelRaw, compressed: kernelCompressed } = loadKernelImage(kernelPath);
  const cmdline = Buffer.from('console=ttyS0 ruvbot.api_port=3000 ruvbot.mode=rvf quiet', 'utf8');
  const kernelHdr = makeKernelHeader(
    kernelRaw.length,
    kernelCompressed.length,
    cmdline.length,
    true  // compressed
  );
  const imgHash = createHash('sha256').update(kernelRaw).digest();
  imgHash.copy(kernelHdr, 0x30, 0, 32);
  // Build ID from first 16 bytes of hash
  imgHash.copy(kernelHdr, 0x50, 0, 16);
  const kernelPayload = Buffer.concat([kernelHdr, kernelCompressed, cmdline]);
  const kSegId = segId++;
  segments.push({ segType: SEG_KERNEL, segId: kSegId, payload: kernelPayload });

  // 2. WASM_SEG — RuvBot runtime bundle
  console.log('  [2/6] Bundling RuvBot runtime...');
  const runtimeBundle = buildRuntimeBundle(pkgDir);
  const wasmHdr = makeWasmHeader(runtimeBundle.length);
  const runtimeHash = createHash('sha256').update(runtimeBundle).digest();
  runtimeHash.copy(wasmHdr, 0x1C, 0, 32);
  const wasmPayload = Buffer.concat([wasmHdr, runtimeBundle]);
  const wSegId = segId++;
  segments.push({ segType: SEG_WASM, segId: wSegId, payload: wasmPayload });
  console.log(`    Runtime: ${runtimeBundle.length} bytes (${(runtimeBundle.length / 1024).toFixed(0)} KB)`);

  // 3. META_SEG — Package metadata
  console.log('  [3/6] Writing package metadata...');
  const metaPayload = buildMetaPayload(pkgDir, {
    rawSize: kernelRaw.length,
    compressedSize: kernelCompressed.length,
  });
  const mSegId = segId++;
  segments.push({ segType: SEG_META, segId: mSegId, payload: metaPayload });

  // 4. PROFILE_SEG — Domain profile
  console.log('  [4/6] Writing domain profile...');
  const profilePayload = buildProfilePayload();
  const pSegId = segId++;
  segments.push({ segType: SEG_PROFILE, segId: pSegId, payload: profilePayload });

  // 5. WITNESS_SEG — Build provenance
  console.log('  [5/6] Writing build provenance...');
  const witnessPayload = buildWitnessPayload(
    imgHash.toString('hex'),
    runtimeHash.toString('hex'),
  );
  const witnSegId = segId++;
  segments.push({ segType: SEG_WITNESS, segId: witnSegId, payload: witnessPayload });

  // 6. MANIFEST_SEG — Segment directory
  console.log('  [6/6] Writing manifest...');
  let currentOffset = 0;
  for (const seg of segments) {
    segDir.push({
      segId: seg.segId,
      offset: currentOffset,
      payloadLen: seg.payload.length,
      segType: seg.segType,
    });
    currentOffset += 64 + seg.payload.length;
  }

  const dirEntrySize = 8 + 8 + 8 + 1;
  const manifestSize = 4 + 2 + 8 + 4 + 1 + 3 + (segDir.length * dirEntrySize) + 4;
  const manifestPayload = Buffer.alloc(manifestSize);
  let mo = 0;
  mo = writeU32LE(manifestPayload, mo, 1);            // epoch
  mo = writeU16LE(manifestPayload, mo, 0);             // dimension
  mo = writeU64LE(manifestPayload, mo, 0);             // total_vectors
  mo = writeU32LE(manifestPayload, mo, segDir.length); // seg_count
  mo = writeU8(manifestPayload, mo, 0x42);             // profile_id
  mo += 3;                                              // reserved

  for (const entry of segDir) {
    mo = writeU64LE(manifestPayload, mo, entry.segId);
    mo = writeU64LE(manifestPayload, mo, entry.offset);
    mo = writeU64LE(manifestPayload, mo, entry.payloadLen);
    mo = writeU8(manifestPayload, mo, entry.segType);
  }
  mo = writeU32LE(manifestPayload, mo, 0);             // del_count

  const manSegId = segId++;
  segments.push({ segType: SEG_MANIFEST, segId: manSegId, payload: manifestPayload });

  // Write all segments
  const allBuffers = [];
  for (const seg of segments) {
    allBuffers.push(makeSegmentHeader(seg.segType, seg.segId, seg.payload.length, seg.payload));
    allBuffers.push(seg.payload);
  }

  const rvfData = Buffer.concat(allBuffers);
  mkdirSync(join(pkgDir, 'kernel'), { recursive: true });
  writeFileSync(outputPath, rvfData);

  // Summary
  const mb = (rvfData.length / (1024 * 1024)).toFixed(2);
  console.log(`\n  RVF assembled: ${outputPath}`);
  console.log(`  Total size: ${mb} MB`);
  console.log(`  Segments: ${segments.length}`);
  console.log(`    KERNEL_SEG  : ${(kernelPayload.length / 1024).toFixed(0)} KB (Linux 6.6.80 bzImage, gzip)`);
  console.log(`    WASM_SEG    : ${(wasmPayload.length / 1024).toFixed(0)} KB (Node.js runtime bundle)`);
  console.log(`    META_SEG    : ${metaPayload.length} bytes`);
  console.log(`    PROFILE_SEG : ${profilePayload.length} bytes`);
  console.log(`    WITNESS_SEG : ${witnessPayload.length} bytes`);
  console.log(`    MANIFEST_SEG: ${manifestPayload.length} bytes`);
  console.log(`\n  Kernel SHA-256: ${imgHash.toString('hex')}`);
  console.log(`  Self-contained: boot with Firecracker, QEMU, or Cloud Hypervisor`);
}

// ─── CLI entry ──────────────────────────────────────────────────────────────

const args = process.argv.slice(2);
let outputPath = 'ruvbot.rvf';
let kernelPath = '';

for (let i = 0; i < args.length; i++) {
  if (args[i] === '--output' && args[i + 1]) { outputPath = args[++i]; }
  else if (args[i] === '--kernel' && args[i + 1]) { kernelPath = args[++i]; }
}

const pkgDir = resolve(__dirname, '..');

// Find kernel: CLI arg > kernel/bzImage > RUVBOT_KERNEL env
if (!kernelPath) {
  const candidates = [
    join(pkgDir, 'kernel', 'bzImage'),
    join(pkgDir, 'kernel', 'vmlinux'),
    '/tmp/linux-6.6.80/arch/x86/boot/bzImage',
  ];
  for (const c of candidates) {
    if (existsSync(c)) { kernelPath = c; break; }
  }
}
if (!kernelPath && process.env.RUVBOT_KERNEL) {
  kernelPath = process.env.RUVBOT_KERNEL;
}
if (!kernelPath) {
  console.error('ERROR: No kernel image found.');
  console.error('Provide one with --kernel /path/to/bzImage or place at kernel/bzImage');
  console.error('\nTo build a minimal kernel:');
  console.error('  wget https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.6.80.tar.xz');
  console.error('  tar xf linux-6.6.80.tar.xz && cd linux-6.6.80');
  console.error('  make tinyconfig');
  console.error('  scripts/config --enable 64BIT --enable VIRTIO --enable VIRTIO_NET \\');
  console.error('    --enable NET --enable INET --enable SERIAL_8250_CONSOLE --enable TTY');
  console.error('  make olddefconfig && make -j$(nproc) bzImage');
  process.exit(1);
}

if (!isAbsolute(outputPath)) {
  outputPath = join(pkgDir, outputPath);
}

assembleRvf(pkgDir, outputPath, kernelPath);
