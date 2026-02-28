/**
 * RVF Seed Decoder - Main Application
 *
 * Loads the rvf-wasm module for segment-level operations on .rvf files.
 * Parses RVQS cognitive seed headers in pure JS (matching the 64-byte
 * binary layout from rvf-types/src/qr_seed.rs).
 */

'use strict';

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

const WASM_PATH = window.RVF_WASM_PATH || './rvf_wasm_bg.wasm';

// RVQS seed constants (from rvf-types/src/qr_seed.rs)
const SEED_MAGIC = 0x52565153; // "RVQS"
const SEED_HEADER_SIZE = 64;
const FLAG_HAS_MICROKERNEL = 0x0001;
const FLAG_HAS_DOWNLOAD    = 0x0002;
const FLAG_SIGNED           = 0x0004;
const FLAG_OFFLINE_CAPABLE  = 0x0008;
const FLAG_ENCRYPTED        = 0x0010;
const FLAG_COMPRESSED       = 0x0020;
const FLAG_HAS_VECTORS      = 0x0040;
const FLAG_STREAM_UPGRADE   = 0x0080;

// Download manifest TLV tags
const DL_TAG_HOST_PRIMARY   = 0x0001;
const DL_TAG_HOST_FALLBACK  = 0x0002;
const DL_TAG_CONTENT_HASH   = 0x0003;
const DL_TAG_TOTAL_SIZE     = 0x0004;
const DL_TAG_LAYER_MANIFEST = 0x0005;
const DL_TAG_SESSION_TOKEN  = 0x0006;
const DL_TAG_TTL            = 0x0007;
const DL_TAG_CERT_PIN       = 0x0008;

// Layer ID names
const LAYER_NAMES = {
  0: 'Level 0 Manifest',
  1: 'Hot Cache',
  2: 'HNSW Layer A',
  3: 'Quant Dict',
  4: 'HNSW Layer B',
  5: 'Full Vectors',
  6: 'HNSW Layer C',
};

// RVF segment magic (from rvf-types constants)
const SEGMENT_MAGIC = 0x52564653; // "RVFS"

// Data type names
const DTYPE_NAMES = {
  0: 'F32',
  1: 'F16',
  2: 'BF16',
  3: 'I8',
  4: 'Binary',
};

// Signature algorithm names
const SIG_ALGO_NAMES = {
  0: 'Ed25519',
  1: 'ML-DSA-65',
  2: 'HMAC-SHA256',
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

let wasmInstance = null;
let wasmMemory = null;
let wasmReady = false;
let scannerStream = null;
let scannerAnimFrame = null;

// ---------------------------------------------------------------------------
// WASM Loader
// ---------------------------------------------------------------------------

async function loadWasm() {
  try {
    setStatus('Loading WASM module...');
    const response = await fetch(WASM_PATH);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }
    const bytes = await response.arrayBuffer();
    const importObject = { env: {} };
    const result = await WebAssembly.instantiate(bytes, importObject);
    wasmInstance = result.instance;
    wasmMemory = wasmInstance.exports.memory;
    wasmReady = true;
    setStatus('WASM loaded -- ready', 'success');
    return true;
  } catch (err) {
    wasmReady = false;
    setStatus(`WASM unavailable: ${err.message}. Falling back to JS-only parsing.`, 'error');
    return false;
  }
}

// ---------------------------------------------------------------------------
// WASM Helpers
// ---------------------------------------------------------------------------

/** Allocate bytes in WASM memory and copy data in. Returns ptr. */
function wasmWrite(data) {
  const ptr = wasmInstance.exports.rvf_alloc(data.length);
  if (ptr === 0) throw new Error('WASM allocation failed');
  const mem = new Uint8Array(wasmMemory.buffer, ptr, data.length);
  mem.set(data);
  return ptr;
}

/** Free WASM memory. */
function wasmFree(ptr, size) {
  wasmInstance.exports.rvf_free(ptr, size);
}

/** Read bytes from WASM memory. */
function wasmRead(ptr, len) {
  return new Uint8Array(wasmMemory.buffer, ptr, len).slice();
}

// ---------------------------------------------------------------------------
// Binary Read Helpers
// ---------------------------------------------------------------------------

function readU16LE(buf, off) {
  return buf[off] | (buf[off + 1] << 8);
}

function readU32LE(buf, off) {
  return (buf[off] | (buf[off + 1] << 8) | (buf[off + 2] << 16) | (buf[off + 3] << 24)) >>> 0;
}

function readU64LE(buf, off) {
  const lo = readU32LE(buf, off);
  const hi = readU32LE(buf, off + 4);
  return lo + hi * 0x100000000;
}

function toHex(bytes, maxLen) {
  const arr = maxLen ? bytes.slice(0, maxLen) : bytes;
  let hex = '';
  for (let i = 0; i < arr.length; i++) {
    hex += arr[i].toString(16).padStart(2, '0');
  }
  if (maxLen && bytes.length > maxLen) {
    hex += '...';
  }
  return hex;
}

function formatBytes(n) {
  if (n < 1024) return n + ' B';
  if (n < 1024 * 1024) return (n / 1024).toFixed(1) + ' KB';
  if (n < 1024 * 1024 * 1024) return (n / (1024 * 1024)).toFixed(2) + ' MB';
  return (n / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
}

function formatTimestamp(ns) {
  if (ns === 0) return '(not set)';
  try {
    const ms = ns / 1e6;
    return new Date(ms).toISOString();
  } catch {
    return '(invalid)';
  }
}

// ---------------------------------------------------------------------------
// Seed Header Parser (Pure JS - matches rvf-types/src/qr_seed.rs layout)
// ---------------------------------------------------------------------------

/**
 * Parse RVQS seed header from raw bytes.
 * @param {Uint8Array} data - Full seed payload (>= 64 bytes)
 * @returns {object} Parsed seed header and manifest
 */
function parseSeedHeader(data) {
  if (data.length < SEED_HEADER_SIZE) {
    throw new Error(`Seed too small: ${data.length} bytes (need >= ${SEED_HEADER_SIZE})`);
  }

  const magic = readU32LE(data, 0x00);
  if (magic !== SEED_MAGIC) {
    throw new Error(
      `Bad magic: 0x${magic.toString(16).padStart(8, '0')} (expected 0x${SEED_MAGIC.toString(16).padStart(8, '0')} "RVQS")`
    );
  }

  const header = {
    magic,
    version: readU16LE(data, 0x04),
    flags: readU16LE(data, 0x06),
    fileId: data.slice(0x08, 0x10),
    totalVectorCount: readU32LE(data, 0x10),
    dimension: readU16LE(data, 0x14),
    baseDtype: data[0x16],
    profileId: data[0x17],
    createdNs: readU64LE(data, 0x18),
    microkernelOffset: readU32LE(data, 0x20),
    microkernelSize: readU32LE(data, 0x24),
    downloadManifestOffset: readU32LE(data, 0x28),
    downloadManifestSize: readU32LE(data, 0x2c),
    sigAlgo: readU16LE(data, 0x30),
    sigLength: readU16LE(data, 0x32),
    totalSeedSize: readU32LE(data, 0x34),
    contentHash: data.slice(0x38, 0x40),
  };

  // Decode flags
  header.flagNames = [];
  const flagDefs = [
    [FLAG_HAS_MICROKERNEL, 'MICROKERNEL'],
    [FLAG_HAS_DOWNLOAD,    'DOWNLOAD'],
    [FLAG_SIGNED,           'SIGNED'],
    [FLAG_OFFLINE_CAPABLE,  'OFFLINE'],
    [FLAG_ENCRYPTED,        'ENCRYPTED'],
    [FLAG_COMPRESSED,       'COMPRESSED'],
    [FLAG_HAS_VECTORS,      'VECTORS'],
    [FLAG_STREAM_UPGRADE,   'STREAM_UPGRADE'],
  ];
  for (const [bit, name] of flagDefs) {
    if (header.flags & bit) header.flagNames.push(name);
  }

  return header;
}

/**
 * Parse download manifest TLV from seed payload.
 * @param {Uint8Array} data - Full seed payload
 * @param {object} header - Parsed header from parseSeedHeader
 * @returns {object} Manifest with hosts, layers, etc.
 */
function parseManifest(data, header) {
  const manifest = {
    hosts: [],
    contentHash: null,
    totalFileSize: null,
    layers: [],
    sessionToken: null,
    tokenTtl: null,
    certPin: null,
  };

  if (!(header.flags & FLAG_HAS_DOWNLOAD) || header.downloadManifestSize === 0) {
    return manifest;
  }

  const start = header.downloadManifestOffset;
  const end = start + header.downloadManifestSize;
  if (end > data.length) return manifest;

  let pos = start;
  while (pos + 4 <= end) {
    const tag = readU16LE(data, pos);
    const length = readU16LE(data, pos + 2);
    pos += 4;

    if (pos + length > end) break;

    const value = data.slice(pos, pos + length);

    switch (tag) {
      case DL_TAG_HOST_PRIMARY:
      case DL_TAG_HOST_FALLBACK: {
        if (length >= 150) {
          const urlLength = readU16LE(value, 0);
          const urlBytes = value.slice(2, 2 + Math.min(urlLength, 128));
          let url = '';
          try { url = new TextDecoder().decode(urlBytes); } catch { /* ignore */ }
          const priority = readU16LE(value, 130);
          const region = readU16LE(value, 132);
          const hostKeyHash = value.slice(134, 150);
          manifest.hosts.push({
            url,
            priority,
            region,
            hostKeyHash,
            isPrimary: tag === DL_TAG_HOST_PRIMARY,
          });
        }
        break;
      }
      case DL_TAG_CONTENT_HASH: {
        if (length >= 32) {
          manifest.contentHash = value.slice(0, 32);
        }
        break;
      }
      case DL_TAG_TOTAL_SIZE: {
        if (length >= 8) {
          manifest.totalFileSize = readU64LE(value, 0);
        }
        break;
      }
      case DL_TAG_LAYER_MANIFEST: {
        if (length > 0) {
          const layerCount = value[0];
          let lpos = 1;
          for (let i = 0; i < layerCount; i++) {
            if (lpos + 27 > value.length) break;
            const layerId = value[lpos];
            const priority = value[lpos + 1];
            const offset = readU32LE(value, lpos + 2);
            const size = readU32LE(value, lpos + 6);
            const contentHash = value.slice(lpos + 10, lpos + 26);
            const required = value[lpos + 26];
            manifest.layers.push({
              layerId,
              name: LAYER_NAMES[layerId] || `Layer ${layerId}`,
              priority,
              offset,
              size,
              contentHash,
              required: required === 1,
            });
            lpos += 27;
          }
        }
        break;
      }
      case DL_TAG_SESSION_TOKEN: {
        if (length >= 16) {
          manifest.sessionToken = value.slice(0, 16);
        }
        break;
      }
      case DL_TAG_TTL: {
        if (length >= 4) {
          manifest.tokenTtl = readU32LE(value, 0);
        }
        break;
      }
      case DL_TAG_CERT_PIN: {
        if (length >= 32) {
          manifest.certPin = value.slice(0, 32);
        }
        break;
      }
      default:
        // Unknown tags are forward-compatible, skip.
        break;
    }

    pos += length;
  }

  return manifest;
}

/**
 * Extract signature bytes from seed payload.
 * @param {Uint8Array} data - Full seed payload
 * @param {object} header - Parsed header
 * @returns {Uint8Array|null} Signature bytes
 */
function extractSignature(data, header) {
  if (!(header.flags & FLAG_SIGNED) || header.sigLength === 0) return null;
  const sigStart = header.totalSeedSize - header.sigLength;
  const sigEnd = header.totalSeedSize;
  if (sigEnd > data.length) return null;
  return data.slice(sigStart, sigEnd);
}

/**
 * Extract microkernel bytes from seed payload.
 * @param {Uint8Array} data - Full seed payload
 * @param {object} header - Parsed header
 * @returns {Uint8Array|null} Microkernel bytes (compressed)
 */
function extractMicrokernel(data, header) {
  if (!(header.flags & FLAG_HAS_MICROKERNEL) || header.microkernelSize === 0) return null;
  const start = header.microkernelOffset;
  const end = start + header.microkernelSize;
  if (end > data.length) return null;
  return data.slice(start, end);
}

// ---------------------------------------------------------------------------
// RVF Segment Parser (uses WASM when available, pure JS fallback)
// ---------------------------------------------------------------------------

/**
 * Parse .rvf file segments.
 * @param {Uint8Array} data - Raw .rvf file bytes
 * @returns {object} Parsed segment info
 */
function parseRvfSegments(data) {
  const result = {
    segmentCount: 0,
    segments: [],
    storeHandle: -1,
    headerValid: false,
  };

  // Try WASM path first
  if (wasmReady) {
    try {
      return parseRvfSegmentsWasm(data);
    } catch (err) {
      // Fall through to JS fallback
    }
  }

  // JS fallback: scan for segment headers
  return parseRvfSegmentsJS(data);
}

function parseRvfSegmentsWasm(data) {
  const bufPtr = wasmWrite(data);
  try {
    // Header verification
    const headerResult = wasmInstance.exports.rvf_verify_header(bufPtr);

    // Segment count
    const segCount = wasmInstance.exports.rvf_segment_count(bufPtr, data.length);

    // Segment info (28 bytes per segment)
    const segments = [];
    const infoSize = 28;
    const infoPtr = wasmInstance.exports.rvf_alloc(infoSize);
    try {
      for (let i = 0; i < segCount; i++) {
        const rc = wasmInstance.exports.rvf_segment_info(bufPtr, data.length, i, infoPtr);
        if (rc === 0) {
          const info = wasmRead(infoPtr, infoSize);
          const segId = readU64LE(info, 0);
          const segType = info[8];
          const payloadLength = readU64LE(info, 12);
          const offset = readU64LE(info, 20);
          segments.push({ segId, segType, payloadLength, offset });
        }
      }
    } finally {
      wasmFree(infoPtr, infoSize);
    }

    // CRC32C verification
    let checksumValid = null;
    if (data.length >= 4) {
      checksumValid = wasmInstance.exports.rvf_verify_checksum(bufPtr, data.length) === 1;
    }

    // Try to open as a store
    let storeHandle = -1;
    try {
      storeHandle = wasmInstance.exports.rvf_store_open(bufPtr, data.length);
    } catch { /* ignore */ }

    return {
      segmentCount: segCount,
      segments,
      headerValid: headerResult === 0,
      checksumValid,
      storeHandle,
    };
  } finally {
    wasmFree(bufPtr, data.length);
  }
}

function parseRvfSegmentsJS(data) {
  const MAGIC_BYTES = [
    SEGMENT_MAGIC & 0xff,
    (SEGMENT_MAGIC >> 8) & 0xff,
    (SEGMENT_MAGIC >> 16) & 0xff,
    (SEGMENT_MAGIC >> 24) & 0xff,
  ];
  const HEADER_SIZE = 64; // rvf-types SEGMENT_HEADER_SIZE
  const segments = [];

  if (data.length < HEADER_SIZE) {
    return { segmentCount: 0, segments, headerValid: false, storeHandle: -1 };
  }

  let i = 0;
  const last = data.length - HEADER_SIZE;

  while (i <= last) {
    if (
      data[i] === MAGIC_BYTES[0] &&
      data[i + 1] === MAGIC_BYTES[1] &&
      data[i + 2] === MAGIC_BYTES[2] &&
      data[i + 3] === MAGIC_BYTES[3]
    ) {
      const version = data[i + 4];
      if (version === 1) {
        const segType = data[i + 5];
        const segId = readU64LE(data, i + 8);
        const payloadLength = readU64LE(data, i + 16);

        segments.push({
          segId,
          segType,
          payloadLength,
          offset: i,
        });

        const total = HEADER_SIZE + payloadLength;
        const next = i + total;
        if (next > i && next <= data.length) {
          i = next;
          continue;
        }
      }
    }
    i++;
  }

  // Check first 4 bytes for magic
  const headerValid =
    data.length >= 4 &&
    readU32LE(data, 0) === SEGMENT_MAGIC;

  return {
    segmentCount: segments.length,
    segments,
    headerValid,
    storeHandle: -1,
  };
}

// Segment type names
const SEG_TYPE_NAMES = {
  0x01: 'Vec',
  0x02: 'HNSW',
  0x03: 'IVF',
  0x04: 'PQ',
  0x05: 'Manifest',
  0x06: 'Metadata',
  0x10: 'WASM',
};

// ---------------------------------------------------------------------------
// Decode Entry Points
// ---------------------------------------------------------------------------

/**
 * Decode a seed (RVQS) or RVF file from raw bytes.
 * Detects type by magic number.
 */
async function decodeSeed(bytes) {
  const data = new Uint8Array(bytes);
  if (data.length < 4) {
    throw new Error('File too small to contain valid data');
  }

  const magic = readU32LE(data, 0);

  if (magic === SEED_MAGIC) {
    // RVQS cognitive seed
    const header = parseSeedHeader(data);
    const manifest = parseManifest(data, header);
    const signature = extractSignature(data, header);
    const microkernel = extractMicrokernel(data, header);
    return {
      type: 'seed',
      header,
      manifest,
      signature,
      microkernel,
      raw: data,
    };
  }

  if (magic === SEGMENT_MAGIC) {
    // Raw .rvf file
    const segInfo = parseRvfSegments(data);
    return {
      type: 'rvf',
      segInfo,
      raw: data,
    };
  }

  // Try as witness bundle (check for witness-specific patterns)
  return decodeWitness(data);
}

/**
 * Decode a witness bundle header.
 * Witness bundles are envelope structures containing evidence chains.
 * We parse the outer framing to show whatever structure is present.
 */
function decodeWitness(data) {
  if (data.length < 4) {
    throw new Error('File too small for witness bundle');
  }

  const magic = readU32LE(data, 0);

  // If this is actually a seed or RVF segment, redirect
  if (magic === SEED_MAGIC || magic === SEGMENT_MAGIC) {
    throw new Error('Not a witness bundle (detected seed or segment magic)');
  }

  // Generic binary inspection for unknown formats
  return {
    type: 'witness',
    size: data.length,
    magic: '0x' + magic.toString(16).padStart(8, '0'),
    raw: data,
    preview: data.slice(0, Math.min(256, data.length)),
  };
}

// ---------------------------------------------------------------------------
// UI Rendering
// ---------------------------------------------------------------------------

function setStatus(msg, cls) {
  const el = document.getElementById('status');
  el.textContent = msg;
  el.className = 'status-bar' + (cls ? ' ' + cls : '');
}

function renderResults(result) {
  const container = document.getElementById('results');
  container.innerHTML = '';

  if (result.type === 'seed') {
    renderSeedResult(container, result);
  } else if (result.type === 'rvf') {
    renderRvfResult(container, result);
  } else if (result.type === 'witness') {
    renderWitnessResult(container, result);
  }
}

function renderSeedResult(container, result) {
  const { header, manifest, signature, microkernel, raw } = result;

  // -- Header Card --
  const headerCard = createCard('Seed Header');
  const grid = document.createElement('dl');
  grid.className = 'info-grid';

  addInfoRow(grid, 'Magic', `0x${header.magic.toString(16).padStart(8, '0')} (RVQS)`);
  addInfoRow(grid, 'Version', header.version.toString());
  addInfoRow(grid, 'File ID', toHex(header.fileId));
  addInfoRow(grid, 'Total Vectors', header.totalVectorCount.toLocaleString());
  addInfoRow(grid, 'Dimension', header.dimension.toString());
  addInfoRow(grid, 'Data Type', DTYPE_NAMES[header.baseDtype] || `0x${header.baseDtype.toString(16)}`);
  addInfoRow(grid, 'Profile ID', header.profileId.toString());
  addInfoRow(grid, 'Created', formatTimestamp(header.createdNs));
  addInfoRow(grid, 'Seed Size', formatBytes(header.totalSeedSize));
  addInfoRow(grid, 'Content Hash', toHex(header.contentHash));

  if (header.flags & FLAG_HAS_MICROKERNEL) {
    addInfoRow(grid, 'Microkernel', `${formatBytes(header.microkernelSize)} @ offset ${header.microkernelOffset}`);
  }

  if (header.flags & FLAG_SIGNED) {
    addInfoRow(grid, 'Signature', `${SIG_ALGO_NAMES[header.sigAlgo] || `algo ${header.sigAlgo}`}, ${header.sigLength} bytes`);
  }

  headerCard.appendChild(grid);

  // Flags badges
  const flagsWrap = document.createElement('div');
  flagsWrap.style.marginTop = '0.75rem';
  const flagsLabel = document.createElement('dt');
  flagsLabel.style.color = 'var(--text-muted)';
  flagsLabel.style.fontSize = '0.85rem';
  flagsLabel.style.marginBottom = '0.35rem';
  flagsLabel.textContent = 'Flags';
  flagsWrap.appendChild(flagsLabel);

  const flagsList = document.createElement('ul');
  flagsList.className = 'flags-list';
  const allFlags = [
    [FLAG_HAS_MICROKERNEL, 'MICROKERNEL'],
    [FLAG_HAS_DOWNLOAD,    'DOWNLOAD'],
    [FLAG_SIGNED,           'SIGNED'],
    [FLAG_OFFLINE_CAPABLE,  'OFFLINE'],
    [FLAG_ENCRYPTED,        'ENCRYPTED'],
    [FLAG_COMPRESSED,       'COMPRESSED'],
    [FLAG_HAS_VECTORS,      'VECTORS'],
    [FLAG_STREAM_UPGRADE,   'STREAM_UPGRADE'],
  ];
  for (const [bit, name] of allFlags) {
    const li = document.createElement('li');
    const badge = document.createElement('span');
    badge.className = 'badge ' + (header.flags & bit ? 'badge-on' : 'badge-off');
    badge.textContent = name;
    li.appendChild(badge);
    flagsList.appendChild(li);
  }
  flagsWrap.appendChild(flagsList);
  headerCard.appendChild(flagsWrap);

  container.appendChild(headerCard);

  // -- Hosts Card --
  if (manifest.hosts.length > 0) {
    const hostsCard = createCard('Download Hosts');
    const table = document.createElement('table');
    table.className = 'data-table';
    table.innerHTML = `
      <thead><tr>
        <th>Type</th><th>URL</th><th>Priority</th><th>Region</th>
      </tr></thead>
    `;
    const tbody = document.createElement('tbody');
    for (const host of manifest.hosts) {
      const tr = document.createElement('tr');
      tr.innerHTML = `
        <td><span class="badge ${host.isPrimary ? 'badge-on' : 'badge-off'}">${host.isPrimary ? 'PRIMARY' : 'FALLBACK'}</span></td>
        <td>${escapeHtml(host.url)}</td>
        <td>${host.priority}</td>
        <td>${host.region}</td>
      `;
      tbody.appendChild(tr);
    }
    table.appendChild(tbody);
    hostsCard.appendChild(table);
    container.appendChild(hostsCard);
  }

  // -- Layers Card --
  if (manifest.layers.length > 0) {
    const layersCard = createCard('Progressive Layers');
    const table = document.createElement('table');
    table.className = 'data-table';
    table.innerHTML = `
      <thead><tr>
        <th>ID</th><th>Name</th><th>Priority</th><th>Size</th><th>Offset</th><th>Required</th><th>Hash</th>
      </tr></thead>
    `;
    const tbody = document.createElement('tbody');
    for (const layer of manifest.layers) {
      const tr = document.createElement('tr');
      tr.innerHTML = `
        <td>${layer.layerId}</td>
        <td>${escapeHtml(layer.name)}</td>
        <td>${layer.priority}</td>
        <td>${formatBytes(layer.size)}</td>
        <td>${layer.offset}</td>
        <td><span class="badge ${layer.required ? 'badge-warn' : 'badge-off'}">${layer.required ? 'YES' : 'no'}</span></td>
        <td class="hex">${toHex(layer.contentHash, 8)}</td>
      `;
      tbody.appendChild(tr);
    }
    table.appendChild(tbody);
    layersCard.appendChild(table);

    if (manifest.totalFileSize !== null) {
      const note = document.createElement('p');
      note.style.cssText = 'margin-top:0.5rem;font-size:0.8rem;color:var(--text-muted)';
      note.textContent = `Total RVF file size: ${formatBytes(manifest.totalFileSize)}`;
      layersCard.appendChild(note);
    }

    container.appendChild(layersCard);
  }

  // -- Manifest extras --
  if (manifest.contentHash || manifest.certPin || manifest.sessionToken || manifest.tokenTtl !== null) {
    const extrasCard = createCard('Manifest Details');
    const grid2 = document.createElement('dl');
    grid2.className = 'info-grid';
    if (manifest.contentHash) {
      addInfoRow(grid2, 'Full Content Hash', toHex(manifest.contentHash));
    }
    if (manifest.certPin) {
      addInfoRow(grid2, 'TLS Cert Pin', toHex(manifest.certPin));
    }
    if (manifest.sessionToken) {
      addInfoRow(grid2, 'Session Token', toHex(manifest.sessionToken));
    }
    if (manifest.tokenTtl !== null) {
      addInfoRow(grid2, 'Token TTL', `${manifest.tokenTtl}s`);
    }
    extrasCard.appendChild(grid2);
    container.appendChild(extrasCard);
  }

  // -- Evidence (signature + microkernel hex) --
  if (signature || microkernel) {
    const evidenceCard = createCard('Evidence');

    if (signature) {
      const details = document.createElement('details');
      details.className = 'evidence-section';
      const summary = document.createElement('summary');
      summary.textContent = `Signature (${SIG_ALGO_NAMES[header.sigAlgo] || 'unknown'}, ${signature.length} bytes)`;
      details.appendChild(summary);
      const pre = document.createElement('div');
      pre.className = 'evidence-hex';
      pre.textContent = formatHexDump(signature);
      details.appendChild(pre);
      evidenceCard.appendChild(details);
    }

    if (microkernel) {
      const details = document.createElement('details');
      details.className = 'evidence-section';
      const summary = document.createElement('summary');
      summary.textContent = `Microkernel (${formatBytes(microkernel.length)}, ${header.flags & FLAG_COMPRESSED ? 'compressed' : 'raw'})`;
      details.appendChild(summary);
      const pre = document.createElement('div');
      pre.className = 'evidence-hex';
      pre.textContent = formatHexDump(microkernel.slice(0, 512));
      if (microkernel.length > 512) {
        const note = document.createElement('p');
        note.style.cssText = 'font-size:0.75rem;color:var(--text-muted);margin-top:0.25rem';
        note.textContent = `Showing first 512 of ${microkernel.length} bytes`;
        details.appendChild(note);
      }
      details.appendChild(pre);
      evidenceCard.appendChild(details);
    }

    container.appendChild(evidenceCard);
  }

  // -- Raw Hex Dump --
  {
    const rawCard = createCard('Raw Payload');
    const details = document.createElement('details');
    details.className = 'evidence-section';
    const summary = document.createElement('summary');
    summary.textContent = `Full hex dump (${formatBytes(raw.length)})`;
    details.appendChild(summary);
    const pre = document.createElement('div');
    pre.className = 'evidence-hex';
    pre.textContent = formatHexDump(raw.slice(0, 1024));
    if (raw.length > 1024) {
      const note = document.createElement('p');
      note.style.cssText = 'font-size:0.75rem;color:var(--text-muted);margin-top:0.25rem';
      note.textContent = `Showing first 1024 of ${raw.length} bytes`;
      details.appendChild(note);
    }
    details.appendChild(pre);
    rawCard.appendChild(details);
    container.appendChild(rawCard);
  }
}

function renderRvfResult(container, result) {
  const { segInfo, raw } = result;

  const overviewCard = createCard('RVF File');
  const grid = document.createElement('dl');
  grid.className = 'info-grid';
  addInfoRow(grid, 'File Size', formatBytes(raw.length));
  addInfoRow(grid, 'Header Valid', segInfo.headerValid ? 'Yes' : 'No');
  addInfoRow(grid, 'Segment Count', segInfo.segmentCount.toString());
  if (segInfo.checksumValid !== undefined && segInfo.checksumValid !== null) {
    addInfoRow(grid, 'Checksum', segInfo.checksumValid ? 'Valid' : 'Invalid');
  }
  overviewCard.appendChild(grid);
  container.appendChild(overviewCard);

  if (segInfo.segments.length > 0) {
    const segsCard = createCard('Segments');
    const table = document.createElement('table');
    table.className = 'data-table';
    table.innerHTML = `
      <thead><tr>
        <th>#</th><th>Type</th><th>Segment ID</th><th>Payload Size</th><th>Offset</th>
      </tr></thead>
    `;
    const tbody = document.createElement('tbody');
    for (let i = 0; i < segInfo.segments.length; i++) {
      const seg = segInfo.segments[i];
      const typeName = SEG_TYPE_NAMES[seg.segType] || `0x${seg.segType.toString(16).padStart(2, '0')}`;
      const tr = document.createElement('tr');
      tr.innerHTML = `
        <td>${i}</td>
        <td><span class="badge badge-on">${escapeHtml(typeName)}</span></td>
        <td class="hex">${seg.segId}</td>
        <td>${formatBytes(seg.payloadLength)}</td>
        <td>${seg.offset}</td>
      `;
      tbody.appendChild(tr);
    }
    table.appendChild(tbody);
    segsCard.appendChild(table);
    container.appendChild(segsCard);
  }

  // Raw hex
  const rawCard = createCard('Raw Data');
  const details = document.createElement('details');
  details.className = 'evidence-section';
  const summary = document.createElement('summary');
  summary.textContent = `Hex dump (${formatBytes(raw.length)})`;
  details.appendChild(summary);
  const pre = document.createElement('div');
  pre.className = 'evidence-hex';
  pre.textContent = formatHexDump(raw.slice(0, 1024));
  details.appendChild(pre);
  rawCard.appendChild(details);
  container.appendChild(rawCard);
}

function renderWitnessResult(container, result) {
  const card = createCard('Witness Bundle');
  const grid = document.createElement('dl');
  grid.className = 'info-grid';
  addInfoRow(grid, 'Size', formatBytes(result.size));
  addInfoRow(grid, 'Magic', result.magic);
  card.appendChild(grid);

  const details = document.createElement('details');
  details.className = 'evidence-section';
  const summary = document.createElement('summary');
  summary.textContent = `Hex preview (${Math.min(256, result.size)} bytes)`;
  details.appendChild(summary);
  const pre = document.createElement('div');
  pre.className = 'evidence-hex';
  pre.textContent = formatHexDump(result.preview);
  details.appendChild(pre);
  card.appendChild(details);

  container.appendChild(card);
}

// ---------------------------------------------------------------------------
// DOM Helpers
// ---------------------------------------------------------------------------

function createCard(title) {
  const card = document.createElement('div');
  card.className = 'result-card';
  const h2 = document.createElement('h2');
  h2.textContent = title;
  card.appendChild(h2);
  return card;
}

function addInfoRow(grid, label, value) {
  const dt = document.createElement('dt');
  dt.textContent = label;
  const dd = document.createElement('dd');
  dd.textContent = value;
  grid.appendChild(dt);
  grid.appendChild(dd);
}

function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

function formatHexDump(bytes) {
  const lines = [];
  for (let i = 0; i < bytes.length; i += 16) {
    const offset = i.toString(16).padStart(8, '0');
    const hexParts = [];
    let ascii = '';
    for (let j = 0; j < 16; j++) {
      if (i + j < bytes.length) {
        hexParts.push(bytes[i + j].toString(16).padStart(2, '0'));
        const ch = bytes[i + j];
        ascii += (ch >= 0x20 && ch <= 0x7e) ? String.fromCharCode(ch) : '.';
      } else {
        hexParts.push('  ');
        ascii += ' ';
      }
    }
    const hex = hexParts.slice(0, 8).join(' ') + '  ' + hexParts.slice(8).join(' ');
    lines.push(`${offset}  ${hex}  |${ascii}|`);
  }
  return lines.join('\n');
}

// ---------------------------------------------------------------------------
// File Input & Drag-and-Drop
// ---------------------------------------------------------------------------

function setupFileHandlers() {
  const dropZone = document.getElementById('drop-zone');
  const fileInput = document.getElementById('file-input');
  const browseBtn = document.getElementById('btn-browse');

  browseBtn.addEventListener('click', () => fileInput.click());

  fileInput.addEventListener('change', (e) => {
    if (e.target.files.length > 0) {
      handleFile(e.target.files[0]);
    }
  });

  dropZone.addEventListener('dragover', (e) => {
    e.preventDefault();
    e.stopPropagation();
    dropZone.classList.add('dragover');
  });

  dropZone.addEventListener('dragleave', (e) => {
    e.preventDefault();
    e.stopPropagation();
    dropZone.classList.remove('dragover');
  });

  dropZone.addEventListener('drop', (e) => {
    e.preventDefault();
    e.stopPropagation();
    dropZone.classList.remove('dragover');
    if (e.dataTransfer.files.length > 0) {
      handleFile(e.dataTransfer.files[0]);
    }
  });

  // Click anywhere in drop zone opens file picker
  dropZone.addEventListener('click', (e) => {
    if (e.target === dropZone || e.target.tagName === 'P') {
      fileInput.click();
    }
  });
}

async function handleFile(file) {
  try {
    setStatus(`Reading ${file.name} (${formatBytes(file.size)})...`);
    const buffer = await file.arrayBuffer();
    const result = await decodeSeed(buffer);
    setStatus(`Decoded ${file.name} as ${result.type.toUpperCase()}`, 'success');
    renderResults(result);
  } catch (err) {
    setStatus(`Error: ${err.message}`, 'error');
    document.getElementById('results').innerHTML = '';
  }
}

// ---------------------------------------------------------------------------
// QR Scanner
// ---------------------------------------------------------------------------

function setupScanner() {
  const scanBtn = document.getElementById('btn-scan');
  const stopBtn = document.getElementById('btn-scan-stop');
  const scannerContainer = document.getElementById('scanner');
  const video = document.getElementById('scanner-video');
  const canvas = document.getElementById('scanner-canvas');

  if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
    scanBtn.disabled = true;
    scanBtn.title = 'Camera not available in this browser';
    return;
  }

  scanBtn.addEventListener('click', async () => {
    try {
      scannerContainer.classList.add('active');
      scanBtn.disabled = true;

      scannerStream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: 'environment' },
      });
      video.srcObject = scannerStream;
      await video.play();

      setStatus('Scanning for QR codes... (point camera at an RVQS QR code)');
      startScanLoop(video, canvas);
    } catch (err) {
      setStatus(`Camera error: ${err.message}`, 'error');
      stopScanner();
    }
  });

  stopBtn.addEventListener('click', stopScanner);
}

function stopScanner() {
  const scanBtn = document.getElementById('btn-scan');
  const scannerContainer = document.getElementById('scanner');
  const video = document.getElementById('scanner-video');

  if (scannerAnimFrame) {
    cancelAnimationFrame(scannerAnimFrame);
    scannerAnimFrame = null;
  }

  if (scannerStream) {
    scannerStream.getTracks().forEach((t) => t.stop());
    scannerStream = null;
  }

  video.srcObject = null;
  scannerContainer.classList.remove('active');
  scanBtn.disabled = false;
}

function startScanLoop(video, canvas) {
  const ctx = canvas.getContext('2d', { willReadFrequently: true });

  function tick() {
    if (!scannerStream) return;

    if (video.readyState >= video.HAVE_ENOUGH_DATA) {
      canvas.width = video.videoWidth;
      canvas.height = video.videoHeight;
      ctx.drawImage(video, 0, 0);

      // Use BarcodeDetector API if available (Chrome/Edge)
      if ('BarcodeDetector' in window) {
        const detector = new BarcodeDetector({ formats: ['qr_code'] });
        detector.detect(canvas).then((barcodes) => {
          if (barcodes.length > 0) {
            handleQrResult(barcodes[0].rawValue);
            return;
          }
        }).catch(() => { /* no detection, keep scanning */ });
      }

      // Try ImageData approach for manual decode
      // (In production, you'd use a QR decode library here.
      //  For this minimal PWA, we rely on BarcodeDetector or raw binary.)
      try {
        const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
        // Binary QR codes encode raw bytes. BarcodeDetector handles this
        // on supported browsers. Without it, the user can export from
        // their QR reader app and load the binary file directly.
        void imageData;
      } catch { /* ignore */ }
    }

    scannerAnimFrame = requestAnimationFrame(tick);
  }

  tick();
}

function handleQrResult(rawValue) {
  stopScanner();

  // QR could contain raw binary or base64-encoded seed
  let bytes;
  try {
    // Try base64 first
    const decoded = atob(rawValue);
    bytes = new Uint8Array(decoded.length);
    for (let i = 0; i < decoded.length; i++) {
      bytes[i] = decoded.charCodeAt(i);
    }
  } catch {
    // Try as raw string bytes
    const encoder = new TextEncoder();
    bytes = encoder.encode(rawValue);
  }

  decodeSeed(bytes.buffer)
    .then((result) => {
      setStatus('Decoded QR seed', 'success');
      renderResults(result);
    })
    .catch((err) => {
      setStatus(`QR decode error: ${err.message}`, 'error');
    });
}

// ---------------------------------------------------------------------------
// Theme Toggle
// ---------------------------------------------------------------------------

function setupTheme() {
  const btn = document.getElementById('btn-theme');
  const stored = localStorage.getItem('rvf-theme');
  if (stored) {
    document.documentElement.setAttribute('data-theme', stored);
    updateThemeLabel(btn, stored);
  }

  btn.addEventListener('click', () => {
    const current = document.documentElement.getAttribute('data-theme');
    const next = current === 'light' ? 'dark' : 'light';
    document.documentElement.setAttribute('data-theme', next);
    localStorage.setItem('rvf-theme', next);
    updateThemeLabel(btn, next);
  });
}

function updateThemeLabel(btn, theme) {
  btn.textContent = theme === 'light' ? 'Dark Mode' : 'Light Mode';
}

// ---------------------------------------------------------------------------
// PWA Registration
// ---------------------------------------------------------------------------

function registerServiceWorker() {
  if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('./sw.js').catch(() => {
      // Service worker registration failed (e.g., file:// protocol)
    });
  }
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

document.addEventListener('DOMContentLoaded', () => {
  setupTheme();
  setupFileHandlers();
  setupScanner();
  registerServiceWorker();
  loadWasm();
});
