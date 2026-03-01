'use strict';

// ── Clinical reference ranges (mirrors REFERENCES in biomarker.rs) ──────────

const BIOMARKER_REFERENCES = Object.freeze([
  { name: 'Total Cholesterol', unit: 'mg/dL', normalLow: 125, normalHigh: 200, criticalLow: 100, criticalHigh: 300, category: 'Lipid' },
  { name: 'LDL', unit: 'mg/dL', normalLow: 50, normalHigh: 100, criticalLow: 25, criticalHigh: 190, category: 'Lipid' },
  { name: 'HDL', unit: 'mg/dL', normalLow: 40, normalHigh: 90, criticalLow: 20, criticalHigh: null, category: 'Lipid' },
  { name: 'Triglycerides', unit: 'mg/dL', normalLow: 35, normalHigh: 150, criticalLow: 20, criticalHigh: 500, category: 'Lipid' },
  { name: 'Fasting Glucose', unit: 'mg/dL', normalLow: 70, normalHigh: 100, criticalLow: 50, criticalHigh: 250, category: 'Metabolic' },
  { name: 'HbA1c', unit: '%', normalLow: 4, normalHigh: 5.7, criticalLow: null, criticalHigh: 9, category: 'Metabolic' },
  { name: 'Homocysteine', unit: 'umol/L', normalLow: 5, normalHigh: 15, criticalLow: null, criticalHigh: 30, category: 'Metabolic' },
  { name: 'Vitamin D', unit: 'ng/mL', normalLow: 30, normalHigh: 80, criticalLow: 10, criticalHigh: 150, category: 'Nutritional' },
  { name: 'CRP', unit: 'mg/L', normalLow: 0, normalHigh: 3, criticalLow: null, criticalHigh: 10, category: 'Inflammatory' },
  { name: 'TSH', unit: 'mIU/L', normalLow: 0.4, normalHigh: 4, criticalLow: 0.1, criticalHigh: 10, category: 'Thyroid' },
  { name: 'Ferritin', unit: 'ng/mL', normalLow: 20, normalHigh: 250, criticalLow: 10, criticalHigh: 1000, category: 'Iron' },
  { name: 'Vitamin B12', unit: 'pg/mL', normalLow: 200, normalHigh: 900, criticalLow: 150, criticalHigh: null, category: 'Nutritional' },
  { name: 'Lp(a)', unit: 'nmol/L', normalLow: 0, normalHigh: 75, criticalLow: null, criticalHigh: 200, category: 'Lipid' },
]);

// ── 20-SNP risk table (mirrors SNPS in biomarker.rs) ────────────────────────

const SNPS = Object.freeze([
  { rsid: 'rs429358',   category: 'Neurological',   wRef: 0, wHet: 0.4,   wAlt: 0.9,  homRef: 'TT', het: 'CT', homAlt: 'CC', maf: 0.14 },
  { rsid: 'rs7412',     category: 'Neurological',   wRef: 0, wHet: -0.15, wAlt: -0.3, homRef: 'CC', het: 'CT', homAlt: 'TT', maf: 0.08 },
  { rsid: 'rs1042522',  category: 'Cancer Risk',    wRef: 0, wHet: 0.25,  wAlt: 0.5,  homRef: 'CC', het: 'CG', homAlt: 'GG', maf: 0.40 },
  { rsid: 'rs80357906', category: 'Cancer Risk',    wRef: 0, wHet: 0.7,   wAlt: 0.95, homRef: 'DD', het: 'DI', homAlt: 'II', maf: 0.003 },
  { rsid: 'rs28897696', category: 'Cancer Risk',    wRef: 0, wHet: 0.3,   wAlt: 0.6,  homRef: 'GG', het: 'AG', homAlt: 'AA', maf: 0.005 },
  { rsid: 'rs11571833', category: 'Cancer Risk',    wRef: 0, wHet: 0.20,  wAlt: 0.5,  homRef: 'AA', het: 'AT', homAlt: 'TT', maf: 0.01 },
  { rsid: 'rs1801133',  category: 'Metabolism',     wRef: 0, wHet: 0.35,  wAlt: 0.7,  homRef: 'GG', het: 'AG', homAlt: 'AA', maf: 0.32 },
  { rsid: 'rs1801131',  category: 'Metabolism',     wRef: 0, wHet: 0.10,  wAlt: 0.25, homRef: 'TT', het: 'GT', homAlt: 'GG', maf: 0.30 },
  { rsid: 'rs4680',     category: 'Neurological',   wRef: 0, wHet: 0.2,   wAlt: 0.45, homRef: 'GG', het: 'AG', homAlt: 'AA', maf: 0.50 },
  { rsid: 'rs1799971',  category: 'Neurological',   wRef: 0, wHet: 0.2,   wAlt: 0.4,  homRef: 'AA', het: 'AG', homAlt: 'GG', maf: 0.15 },
  { rsid: 'rs762551',   category: 'Metabolism',     wRef: 0, wHet: 0.15,  wAlt: 0.35, homRef: 'AA', het: 'AC', homAlt: 'CC', maf: 0.37 },
  { rsid: 'rs4988235',  category: 'Metabolism',     wRef: 0, wHet: 0.05,  wAlt: 0.15, homRef: 'AA', het: 'AG', homAlt: 'GG', maf: 0.24 },
  { rsid: 'rs53576',    category: 'Neurological',   wRef: 0, wHet: 0.1,   wAlt: 0.25, homRef: 'GG', het: 'AG', homAlt: 'AA', maf: 0.35 },
  { rsid: 'rs6311',     category: 'Neurological',   wRef: 0, wHet: 0.15,  wAlt: 0.3,  homRef: 'CC', het: 'CT', homAlt: 'TT', maf: 0.45 },
  { rsid: 'rs1800497',  category: 'Neurological',   wRef: 0, wHet: 0.25,  wAlt: 0.5,  homRef: 'GG', het: 'AG', homAlt: 'AA', maf: 0.20 },
  { rsid: 'rs4363657',  category: 'Cardiovascular', wRef: 0, wHet: 0.35,  wAlt: 0.7,  homRef: 'TT', het: 'CT', homAlt: 'CC', maf: 0.15 },
  { rsid: 'rs1800566',  category: 'Cancer Risk',    wRef: 0, wHet: 0.15,  wAlt: 0.30, homRef: 'CC', het: 'CT', homAlt: 'TT', maf: 0.22 },
  { rsid: 'rs10455872', category: 'Cardiovascular', wRef: 0, wHet: 0.40,  wAlt: 0.75, homRef: 'AA', het: 'AG', homAlt: 'GG', maf: 0.07 },
  { rsid: 'rs3798220',  category: 'Cardiovascular', wRef: 0, wHet: 0.35,  wAlt: 0.65, homRef: 'TT', het: 'CT', homAlt: 'CC', maf: 0.02 },
  { rsid: 'rs11591147', category: 'Cardiovascular', wRef: 0, wHet: -0.30, wAlt: -0.55, homRef: 'GG', het: 'GT', homAlt: 'TT', maf: 0.024 },
]);

// ── Gene-gene interactions (mirrors INTERACTIONS in biomarker.rs) ────────────

const INTERACTIONS = Object.freeze([
  { rsidA: 'rs4680',    rsidB: 'rs1799971', modifier: 1.4,  category: 'Neurological' },
  { rsidA: 'rs1801133', rsidB: 'rs1801131', modifier: 1.3,  category: 'Metabolism' },
  { rsidA: 'rs429358',  rsidB: 'rs1042522', modifier: 1.2,  category: 'Cancer Risk' },
  { rsidA: 'rs80357906',rsidB: 'rs1042522', modifier: 1.5,  category: 'Cancer Risk' },
  { rsidA: 'rs1801131', rsidB: 'rs4680',    modifier: 1.25, category: 'Neurological' },
  { rsidA: 'rs1800497', rsidB: 'rs4680',    modifier: 1.2,  category: 'Neurological' },
]);

const CAT_ORDER = ['Cancer Risk', 'Cardiovascular', 'Neurological', 'Metabolism'];
const NUM_ONEHOT_SNPS = 17;

// ── Helpers ──────────────────────────────────────────────────────────────────

function genotypeCode(snp, gt) {
  if (gt === snp.homRef) return 0;
  if (gt.length === 2 && gt[0] !== gt[1]) return 1;
  return 2;
}

function snpWeight(snp, code) {
  return code === 0 ? snp.wRef : code === 1 ? snp.wHet : snp.wAlt;
}

// Pre-built rsid -> index lookup (O(1) instead of O(n) findIndex)
const RSID_INDEX = new Map();
for (let i = 0; i < SNPS.length; i++) RSID_INDEX.set(SNPS[i].rsid, i);

// Pre-cache LPA SNP references to avoid repeated iteration
const LPA_SNPS = SNPS.filter(s => s.rsid === 'rs10455872' || s.rsid === 'rs3798220');

function snpIndex(rsid) {
  const idx = RSID_INDEX.get(rsid);
  return idx !== undefined ? idx : -1;
}

function isNonRef(genotypes, rsid) {
  const idx = RSID_INDEX.get(rsid);
  if (idx === undefined) return false;
  const gt = genotypes.get(rsid);
  return gt !== undefined && gt !== SNPS[idx].homRef;
}

function interactionMod(genotypes, ix) {
  return (isNonRef(genotypes, ix.rsidA) && isNonRef(genotypes, ix.rsidB)) ? ix.modifier : 1.0;
}

// Pre-compute category metadata (mirrors category_meta() in Rust)
const CATEGORY_META = CAT_ORDER.map(cat => {
  let maxPossible = 0;
  let expectedCount = 0;
  for (const snp of SNPS) {
    if (snp.category === cat) {
      maxPossible += Math.max(snp.wAlt, 0);
      expectedCount++;
    }
  }
  return { name: cat, maxPossible: Math.max(maxPossible, 1), expectedCount };
});

// Mulberry32 PRNG — deterministic, fast, no dependencies
function mulberry32(seed) {
  let t = (seed + 0x6D2B79F5) | 0;
  return function () {
    t = (t + 0x6D2B79F5) | 0;
    let z = t ^ (t >>> 15);
    z = Math.imul(z | 1, z);
    z ^= z + Math.imul(z ^ (z >>> 7), z | 61);
    return ((z ^ (z >>> 14)) >>> 0) / 4294967296;
  };
}

// ── Simplified MTHFR/pain scoring (mirrors health.rs analysis functions) ────

function analyzeMthfr(genotypes) {
  let score = 0;
  const gt677 = genotypes.get('rs1801133');
  const gt1298 = genotypes.get('rs1801131');
  if (gt677) {
    const code = genotypeCode(SNPS[6], gt677);
    score += code;
  }
  if (gt1298) {
    const code = genotypeCode(SNPS[7], gt1298);
    score += code;
  }
  return { score };
}

function analyzePain(genotypes) {
  const gtComt = genotypes.get('rs4680');
  const gtOprm1 = genotypes.get('rs1799971');
  if (!gtComt || !gtOprm1) return null;
  const comtCode = genotypeCode(SNPS[8], gtComt);
  const oprm1Code = genotypeCode(SNPS[9], gtOprm1);
  return { score: comtCode + oprm1Code };
}

// ── Public API ───────────────────────────────────────────────────────────────

function biomarkerReferences() {
  return BIOMARKER_REFERENCES;
}

function zScore(value, ref_) {
  const mid = (ref_.normalLow + ref_.normalHigh) / 2;
  const halfRange = (ref_.normalHigh - ref_.normalLow) / 2;
  if (halfRange === 0) return 0;
  return (value - mid) / halfRange;
}

function classifyBiomarker(value, ref_) {
  if (ref_.criticalLow !== null && value < ref_.criticalLow) return 'CriticalLow';
  if (value < ref_.normalLow) return 'Low';
  if (ref_.criticalHigh !== null && value > ref_.criticalHigh) return 'CriticalHigh';
  if (value > ref_.normalHigh) return 'High';
  return 'Normal';
}

function computeRiskScores(genotypes) {
  const catScores = new Map(); // category -> { raw, variants, count }

  for (const snp of SNPS) {
    const gt = genotypes.get(snp.rsid);
    if (gt === undefined) continue;
    const code = genotypeCode(snp, gt);
    const w = snpWeight(snp, code);
    if (!catScores.has(snp.category)) {
      catScores.set(snp.category, { raw: 0, variants: [], count: 0 });
    }
    const entry = catScores.get(snp.category);
    entry.raw += w;
    entry.count++;
    if (code > 0) entry.variants.push(snp.rsid);
  }

  for (const inter of INTERACTIONS) {
    const m = interactionMod(genotypes, inter);
    if (m > 1.0 && catScores.has(inter.category)) {
      catScores.get(inter.category).raw *= m;
    }
  }

  const categoryScores = {};
  for (const cm of CATEGORY_META) {
    const entry = catScores.get(cm.name) || { raw: 0, variants: [], count: 0 };
    const score = Math.min(Math.max(entry.raw / cm.maxPossible, 0), 1);
    const confidence = entry.count > 0 ? Math.min(entry.count / Math.max(cm.expectedCount, 1), 1) : 0;
    categoryScores[cm.name] = {
      category: cm.name,
      score,
      confidence,
      contributingVariants: entry.variants,
    };
  }

  let ws = 0, cs = 0;
  for (const c of Object.values(categoryScores)) {
    ws += c.score * c.confidence;
    cs += c.confidence;
  }
  const globalRiskScore = cs > 0 ? ws / cs : 0;

  const profile = {
    subjectId: '',
    timestamp: 0,
    categoryScores,
    globalRiskScore,
    profileVector: null,
    biomarkerValues: {},
  };
  profile.profileVector = encodeProfileVectorWithGenotypes(profile, genotypes);
  return profile;
}

function encodeProfileVector(profile) {
  return encodeProfileVectorWithGenotypes(profile, new Map());
}

function encodeProfileVectorWithGenotypes(profile, genotypes) {
  const v = new Float32Array(64);

  // Dims 0..50: one-hot genotype encoding (first 17 SNPs x 3 = 51 dims)
  for (let i = 0; i < NUM_ONEHOT_SNPS; i++) {
    const snp = SNPS[i];
    const gt = genotypes.get(snp.rsid);
    const code = gt !== undefined ? genotypeCode(snp, gt) : 0;
    v[i * 3 + code] = 1.0;
  }

  // Dims 51..54: category scores
  for (let j = 0; j < CAT_ORDER.length; j++) {
    const cs = profile.categoryScores[CAT_ORDER[j]];
    v[51 + j] = cs ? cs.score : 0;
  }
  v[55] = profile.globalRiskScore;

  // Dims 56..59: first 4 interaction modifiers
  for (let j = 0; j < 4; j++) {
    const m = interactionMod(genotypes, INTERACTIONS[j]);
    v[56 + j] = m > 1 ? m - 1 : 0;
  }

  // Dims 60..63: derived clinical scores
  v[60] = analyzeMthfr(genotypes).score / 4;
  const pain = analyzePain(genotypes);
  v[61] = pain ? pain.score / 4 : 0;
  const apoeGt = genotypes.get('rs429358');
  v[62] = apoeGt !== undefined ? genotypeCode(SNPS[0], apoeGt) / 2 : 0;

  // LPA composite: average of rs10455872 + rs3798220 genotype codes (cached)
  let lpaSum = 0, lpaCount = 0;
  for (const snp of LPA_SNPS) {
    const gt = genotypes.get(snp.rsid);
    if (gt !== undefined) {
      lpaSum += genotypeCode(snp, gt) / 2;
      lpaCount++;
    }
  }
  v[63] = lpaCount > 0 ? lpaSum / 2 : 0;

  // L2-normalize
  let norm = 0;
  for (let i = 0; i < 64; i++) norm += v[i] * v[i];
  norm = Math.sqrt(norm);
  if (norm > 0) for (let i = 0; i < 64; i++) v[i] /= norm;

  return v;
}

function randomGenotype(rng, snp) {
  const p = snp.maf;
  const q = 1 - p;
  const r = rng();
  if (r < q * q) return snp.homRef;
  if (r < q * q + 2 * p * q) return snp.het;
  return snp.homAlt;
}

function generateSyntheticPopulation(count, seed) {
  const rng = mulberry32(seed);
  const pop = [];

  for (let i = 0; i < count; i++) {
    const genotypes = new Map();
    for (const snp of SNPS) {
      genotypes.set(snp.rsid, randomGenotype(rng, snp));
    }

    const profile = computeRiskScores(genotypes);
    profile.subjectId = `SYN-${String(i).padStart(6, '0')}`;
    profile.timestamp = 1700000000 + i;

    const mthfrScore = analyzeMthfr(genotypes).score;
    const apoeCode = genotypes.get('rs429358') ? genotypeCode(SNPS[0], genotypes.get('rs429358')) : 0;
    const nqo1Idx = RSID_INDEX.get('rs1800566');
    const nqo1Code = genotypes.get('rs1800566') ? genotypeCode(SNPS[nqo1Idx], genotypes.get('rs1800566')) : 0;

    let lpaRisk = 0;
    for (const snp of LPA_SNPS) {
      const gt = genotypes.get(snp.rsid);
      if (gt) lpaRisk += genotypeCode(snp, gt);
    }

    const pcsk9Idx = RSID_INDEX.get('rs11591147');
    const pcsk9Code = genotypes.get('rs11591147') ? genotypeCode(SNPS[pcsk9Idx], genotypes.get('rs11591147')) : 0;

    for (const bref of BIOMARKER_REFERENCES) {
      const mid = (bref.normalLow + bref.normalHigh) / 2;
      const sd = (bref.normalHigh - bref.normalLow) / 4;
      let val = mid + (rng() * 3 - 1.5) * sd;

      // Gene->biomarker correlations (mirrors Rust)
      const nm = bref.name;
      if (nm === 'Homocysteine' && mthfrScore >= 2) val += sd * (mthfrScore - 1);
      if ((nm === 'Total Cholesterol' || nm === 'LDL') && apoeCode > 0) val += sd * 0.5 * apoeCode;
      if (nm === 'HDL' && apoeCode > 0) val -= sd * 0.3 * apoeCode;
      if (nm === 'Triglycerides' && apoeCode > 0) val += sd * 0.4 * apoeCode;
      if (nm === 'Vitamin B12' && mthfrScore >= 2) val -= sd * 0.4;
      if (nm === 'CRP' && nqo1Code === 2) val += sd * 0.3;
      if (nm === 'Lp(a)' && lpaRisk > 0) val += sd * 1.5 * lpaRisk;
      if ((nm === 'LDL' || nm === 'Total Cholesterol') && pcsk9Code > 0) val -= sd * 0.6 * pcsk9Code;

      val = Math.max(val, bref.criticalLow || 0, 0);
      if (bref.criticalHigh !== null) val = Math.min(val, bref.criticalHigh * 1.2);
      profile.biomarkerValues[bref.name] = Math.round(val * 10) / 10;
    }
    pop.push(profile);
  }
  return pop;
}

module.exports = {
  BIOMARKER_REFERENCES,
  SNPS,
  INTERACTIONS,
  CAT_ORDER,
  biomarkerReferences,
  zScore,
  classifyBiomarker,
  computeRiskScores,
  encodeProfileVector,
  generateSyntheticPopulation,
};
