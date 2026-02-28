const { platform, arch } = process;

// Platform-specific native binary packages
const platformMap = {
  'linux': {
    'x64': '@ruvector/rvdna-linux-x64-gnu',
    'arm64': '@ruvector/rvdna-linux-arm64-gnu'
  },
  'darwin': {
    'x64': '@ruvector/rvdna-darwin-x64',
    'arm64': '@ruvector/rvdna-darwin-arm64'
  },
  'win32': {
    'x64': '@ruvector/rvdna-win32-x64-msvc'
  }
};

function loadNativeModule() {
  const platformPackage = platformMap[platform]?.[arch];

  if (!platformPackage) {
    throw new Error(
      `Unsupported platform: ${platform}-${arch}\n` +
      `@ruvector/rvdna native bindings are available for:\n` +
      `- Linux (x64, ARM64)\n` +
      `- macOS (x64, ARM64)\n` +
      `- Windows (x64)\n\n` +
      `For other platforms, use the WASM build: npm install @ruvector/rvdna-wasm`
    );
  }

  try {
    return require(platformPackage);
  } catch (error) {
    if (error.code === 'MODULE_NOT_FOUND') {
      throw new Error(
        `Native module not found for ${platform}-${arch}\n` +
        `Please install: npm install ${platformPackage}\n` +
        `Or reinstall @ruvector/rvdna to get optional dependencies`
      );
    }
    throw error;
  }
}

// Try native first, fall back to pure JS shim with basic functionality
let nativeModule;
try {
  nativeModule = loadNativeModule();
} catch (e) {
  // Native bindings not available — provide JS shim for basic operations
  nativeModule = null;
}

// -------------------------------------------------------------------
// Public API — wraps native bindings or provides JS fallbacks
// -------------------------------------------------------------------

/**
 * Encode a DNA string to 2-bit packed bytes (4 bases per byte).
 * A=00, C=01, G=10, T=11. Returns a Buffer.
 */
function encode2bit(sequence) {
  if (nativeModule?.encode2bit) return nativeModule.encode2bit(sequence);

  // JS fallback
  const map = { A: 0, C: 1, G: 2, T: 3, N: 0 };
  const len = sequence.length;
  const buf = Buffer.alloc(Math.ceil(len / 4));
  for (let i = 0; i < len; i++) {
    const byteIdx = i >> 2;
    const bitOff = 6 - (i & 3) * 2;
    buf[byteIdx] |= (map[sequence[i]] || 0) << bitOff;
  }
  return buf;
}

/**
 * Decode 2-bit packed bytes back to a DNA string.
 */
function decode2bit(buffer, length) {
  if (nativeModule?.decode2bit) return nativeModule.decode2bit(buffer, length);

  const bases = ['A', 'C', 'G', 'T'];
  let result = '';
  for (let i = 0; i < length; i++) {
    const byteIdx = i >> 2;
    const bitOff = 6 - (i & 3) * 2;
    result += bases[(buffer[byteIdx] >> bitOff) & 3];
  }
  return result;
}

/**
 * Translate a DNA string to a protein amino acid string.
 */
function translateDna(sequence) {
  if (nativeModule?.translateDna) return nativeModule.translateDna(sequence);

  // JS fallback — standard genetic code
  const codons = {
    'TTT':'F','TTC':'F','TTA':'L','TTG':'L','CTT':'L','CTC':'L','CTA':'L','CTG':'L',
    'ATT':'I','ATC':'I','ATA':'I','ATG':'M','GTT':'V','GTC':'V','GTA':'V','GTG':'V',
    'TCT':'S','TCC':'S','TCA':'S','TCG':'S','CCT':'P','CCC':'P','CCA':'P','CCG':'P',
    'ACT':'T','ACC':'T','ACA':'T','ACG':'T','GCT':'A','GCC':'A','GCA':'A','GCG':'A',
    'TAT':'Y','TAC':'Y','TAA':'*','TAG':'*','CAT':'H','CAC':'H','CAA':'Q','CAG':'Q',
    'AAT':'N','AAC':'N','AAA':'K','AAG':'K','GAT':'D','GAC':'D','GAA':'E','GAG':'E',
    'TGT':'C','TGC':'C','TGA':'*','TGG':'W','CGT':'R','CGC':'R','CGA':'R','CGG':'R',
    'AGT':'S','AGC':'S','AGA':'R','AGG':'R','GGT':'G','GGC':'G','GGA':'G','GGG':'G',
  };
  let protein = '';
  for (let i = 0; i + 2 < sequence.length; i += 3) {
    const codon = sequence.slice(i, i + 3).toUpperCase();
    const aa = codons[codon] || 'X';
    if (aa === '*') break;
    protein += aa;
  }
  return protein;
}

/**
 * Compute cosine similarity between two numeric arrays.
 */
function cosineSimilarity(a, b) {
  if (nativeModule?.cosineSimilarity) return nativeModule.cosineSimilarity(a, b);

  let dot = 0, magA = 0, magB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    magA += a[i] * a[i];
    magB += b[i] * b[i];
  }
  magA = Math.sqrt(magA);
  magB = Math.sqrt(magB);
  return (magA && magB) ? dot / (magA * magB) : 0;
}

/**
 * Convert a FASTA sequence string to .rvdna binary format.
 * Returns a Buffer with the complete .rvdna file contents.
 */
function fastaToRvdna(sequence, options = {}) {
  if (nativeModule?.fastaToRvdna) {
    return nativeModule.fastaToRvdna(sequence, options.k || 11, options.dims || 512, options.blockSize || 500);
  }
  throw new Error('fastaToRvdna requires native bindings. Install the platform-specific package.');
}

/**
 * Read a .rvdna file from a Buffer. Returns parsed sections.
 */
function readRvdna(buffer) {
  if (nativeModule?.readRvdna) return nativeModule.readRvdna(buffer);
  throw new Error('readRvdna requires native bindings. Install the platform-specific package.');
}

/**
 * Check if native bindings are available.
 */
function isNativeAvailable() {
  return nativeModule !== null;
}

// -------------------------------------------------------------------
// 23andMe Genotyping Pipeline (pure JS — mirrors Rust rvdna::genotyping)
// -------------------------------------------------------------------

/**
 * Normalize a genotype string: uppercase, trim, sort allele pair.
 * "ag" → "AG", "TC" → "CT", "DI" → "DI"
 */
function normalizeGenotype(gt) {
  gt = gt.trim().toUpperCase();
  if (gt.length === 2 && gt[0] > gt[1]) {
    return gt[1] + gt[0];
  }
  return gt;
}

/**
 * Parse a 23andMe raw data file (v4/v5 tab-separated format).
 * @param {string} text - Raw file contents
 * @returns {{ snps: Map<string,object>, totalMarkers: number, noCalls: number, chrCounts: Map<string,number>, build: string }}
 */
function parse23andMe(text) {
  const snps = new Map();
  const chrCounts = new Map();
  let total = 0, noCalls = 0;
  let build = 'Unknown';

  for (const line of text.split('\n')) {
    if (line.startsWith('#')) {
      const lower = line.toLowerCase();
      if (lower.includes('build 37') || lower.includes('grch37') || lower.includes('hg19')) build = 'GRCh37';
      else if (lower.includes('build 38') || lower.includes('grch38') || lower.includes('hg38')) build = 'GRCh38';
      continue;
    }
    if (!line.trim()) continue;
    const parts = line.split('\t');
    if (parts.length < 4) continue;
    const [rsid, chrom, posStr, genotype] = parts;
    total++;
    if (genotype === '--') { noCalls++; continue; }
    const pos = parseInt(posStr, 10) || 0;
    const normGt = normalizeGenotype(genotype);
    chrCounts.set(chrom, (chrCounts.get(chrom) || 0) + 1);
    snps.set(rsid, { rsid, chromosome: chrom, position: pos, genotype: normGt });
  }

  if (total === 0) throw new Error('No markers found in file');
  return { snps, totalMarkers: total, noCalls, chrCounts, build };
}

// CYP defining variant tables
const CYP2D6_DEFS = [
  { rsid: 'rs3892097',  allele: '*4',  alt: 'T', isDel: false, activity: 0.0, fn: 'No function (splicing defect)' },
  { rsid: 'rs35742686', allele: '*3',  alt: '-', isDel: true,  activity: 0.0, fn: 'No function (frameshift)' },
  { rsid: 'rs5030655',  allele: '*6',  alt: '-', isDel: true,  activity: 0.0, fn: 'No function (frameshift)' },
  { rsid: 'rs1065852',  allele: '*10', alt: 'T', isDel: false, activity: 0.5, fn: 'Decreased function' },
  { rsid: 'rs28371725', allele: '*41', alt: 'T', isDel: false, activity: 0.5, fn: 'Decreased function' },
  { rsid: 'rs28371706', allele: '*17', alt: 'T', isDel: false, activity: 0.5, fn: 'Decreased function' },
];

const CYP2C19_DEFS = [
  { rsid: 'rs4244285',  allele: '*2',  alt: 'A', isDel: false, activity: 0.0, fn: 'No function (splicing defect)' },
  { rsid: 'rs4986893',  allele: '*3',  alt: 'A', isDel: false, activity: 0.0, fn: 'No function (premature stop)' },
  { rsid: 'rs12248560', allele: '*17', alt: 'T', isDel: false, activity: 1.5, fn: 'Increased function' },
];

/**
 * Call a CYP diplotype from a genotype map.
 * @param {string} gene - Gene name (e.g., "CYP2D6")
 * @param {object[]} defs - Defining variant table
 * @param {Map<string,string>} gts - rsid → genotype map
 */
function callCypDiplotype(gene, defs, gts) {
  const alleles = [];
  const details = [];
  const notes = [];
  let genotyped = 0, matched = 0;

  for (const def of defs) {
    const gt = gts.get(def.rsid);
    if (gt !== undefined) {
      genotyped++;
      if (def.isDel) {
        if (gt === 'DD') { matched++; alleles.push([def.allele, def.activity], [def.allele, def.activity]); details.push(`  ${def.rsid}: ${gt} -> homozygous ${def.allele} (${def.fn})`); }
        else if (gt === 'DI') { matched++; alleles.push([def.allele, def.activity]); details.push(`  ${def.rsid}: ${gt} -> heterozygous ${def.allele} (${def.fn})`); }
        else { details.push(`  ${def.rsid}: ${gt} -> reference (no ${def.allele})`); }
      } else {
        const hom = def.alt + def.alt;
        if (gt === hom) { matched++; alleles.push([def.allele, def.activity], [def.allele, def.activity]); details.push(`  ${def.rsid}: ${gt} -> homozygous ${def.allele} (${def.fn})`); }
        else if (gt.includes(def.alt)) { matched++; alleles.push([def.allele, def.activity]); details.push(`  ${def.rsid}: ${gt} -> heterozygous ${def.allele} (${def.fn})`); }
        else { details.push(`  ${def.rsid}: ${gt} -> reference (no ${def.allele})`); }
      }
    } else {
      details.push(`  ${def.rsid}: not genotyped`);
    }
  }

  let confidence;
  if (genotyped === 0) confidence = 'Unsupported';
  else if (matched >= 2 && genotyped * 2 >= defs.length) confidence = 'Strong';
  else if ((matched >= 1 && genotyped >= 2) || genotyped * 2 >= defs.length) confidence = 'Moderate';
  else confidence = 'Weak';

  if (confidence === 'Unsupported') notes.push('Panel lacks all defining variants for this gene.');
  if (confidence === 'Weak') notes.push(`Only ${genotyped}/${defs.length} defining rsids genotyped; call unreliable.`);
  notes.push('No phase or CNV resolution from genotyping array.');

  while (alleles.length < 2) alleles.push(['*1', 1.0]);
  const activity = alleles[0][1] + alleles[1][1];
  let phenotype;
  if (activity > 2.0) phenotype = 'UltraRapid';
  else if (activity >= 1.0) phenotype = 'Normal';
  else if (activity >= 0.5) phenotype = 'Intermediate';
  else phenotype = 'Poor';

  return {
    gene, allele1: alleles[0][0], allele2: alleles[1][0],
    activity, phenotype, confidence,
    rsidsGenotyped: genotyped, rsidsMatched: matched, rsidsTotal: defs.length,
    notes, details,
  };
}

/** Call CYP2D6 diplotype */
function callCyp2d6(gts) { return callCypDiplotype('CYP2D6', CYP2D6_DEFS, gts); }

/** Call CYP2C19 diplotype */
function callCyp2c19(gts) { return callCypDiplotype('CYP2C19', CYP2C19_DEFS, gts); }

/**
 * Determine APOE genotype from rs429358 + rs7412.
 * @param {Map<string,string>} gts
 */
function determineApoe(gts) {
  const gt1 = gts.get('rs429358') || '';
  const gt2 = gts.get('rs7412') || '';
  if (!gt1 || !gt2) return { genotype: 'Unable to determine (missing data)', rs429358: gt1, rs7412: gt2 };
  const e4 = (gt1.match(/C/g) || []).length;
  const e2 = (gt2.match(/T/g) || []).length;
  const geno = {
    '0,0': 'e3/e3 (most common, baseline risk)',
    '0,1': 'e2/e3 (PROTECTIVE - reduced Alzheimer\'s risk)',
    '0,2': 'e2/e2 (protective; monitor for type III hyperlipoproteinemia)',
    '1,0': 'e3/e4 (increased Alzheimer\'s risk ~3x)',
    '1,1': 'e2/e4 (mixed - e2 partially offsets e4 risk)',
  }[`${e4},${e2}`] || (e4 >= 2 ? 'e4/e4 (significantly increased Alzheimer\'s risk ~12x)' : `Unusual: rs429358=${gt1}, rs7412=${gt2}`);
  return { genotype: geno, rs429358: gt1, rs7412: gt2 };
}

/**
 * Run the full 23andMe analysis pipeline.
 * @param {string} text - Raw 23andMe file contents
 * @returns {object} Full analysis result
 */
function analyze23andMe(text) {
  const data = parse23andMe(text);
  const gts = new Map();
  for (const [rsid, snp] of data.snps) gts.set(rsid, snp.genotype);

  const cyp2d6 = callCyp2d6(gts);
  const cyp2c19 = callCyp2c19(gts);
  const apoe = determineApoe(gts);

  // Variant classification
  let homozygous = 0, heterozygous = 0, indels = 0;
  const isNuc = c => 'ACGT'.includes(c);
  for (const snp of data.snps.values()) {
    const g = snp.genotype;
    if (g.length === 2) {
      if (isNuc(g[0]) && isNuc(g[1])) { g[0] === g[1] ? homozygous++ : heterozygous++; }
      else indels++;
    }
  }

  return {
    data: { ...data, snps: Object.fromEntries(data.snps), chrCounts: Object.fromEntries(data.chrCounts) },
    cyp2d6, cyp2c19, apoe,
    homozygous, heterozygous, indels,
    hetRatio: data.totalMarkers - data.noCalls > 0 ? heterozygous / (data.totalMarkers - data.noCalls) * 100 : 0,
  };
}

// -------------------------------------------------------------------
// Biomarker Analysis Engine (v0.3.0 — mirrors biomarker.rs + biomarker_stream.rs)
// -------------------------------------------------------------------

const biomarkerModule = require('./src/biomarker');
const streamModule = require('./src/stream');

module.exports = {
  // Original API
  encode2bit,
  decode2bit,
  translateDna,
  cosineSimilarity,
  fastaToRvdna,
  readRvdna,
  isNativeAvailable,

  // 23andMe Genotyping API (v0.2.0)
  normalizeGenotype,
  parse23andMe,
  callCyp2d6,
  callCyp2c19,
  determineApoe,
  analyze23andMe,

  // Biomarker Risk Scoring Engine (v0.3.0)
  biomarkerReferences: biomarkerModule.biomarkerReferences,
  zScore: biomarkerModule.zScore,
  classifyBiomarker: biomarkerModule.classifyBiomarker,
  computeRiskScores: biomarkerModule.computeRiskScores,
  encodeProfileVector: biomarkerModule.encodeProfileVector,
  generateSyntheticPopulation: biomarkerModule.generateSyntheticPopulation,
  BIOMARKER_REFERENCES: biomarkerModule.BIOMARKER_REFERENCES,
  SNPS: biomarkerModule.SNPS,
  INTERACTIONS: biomarkerModule.INTERACTIONS,
  CAT_ORDER: biomarkerModule.CAT_ORDER,

  // Streaming Biomarker Processor (v0.3.0)
  RingBuffer: streamModule.RingBuffer,
  StreamProcessor: streamModule.StreamProcessor,
  generateReadings: streamModule.generateReadings,
  defaultStreamConfig: streamModule.defaultStreamConfig,
  BIOMARKER_DEFS: streamModule.BIOMARKER_DEFS,

  // Re-export native module for advanced use
  native: nativeModule,
};
