/**
 * DocsView — Documentation with sidebar navigation and scroll-spy.
 */

interface Section {
  id: string;
  label: string;
  icon: string;
  children?: { id: string; label: string }[];
}

const SECTIONS: Section[] = [
  { id: 'overview', label: 'Overview', icon: '\u2302',
    children: [{ id: 'what-is-rvf', label: 'What is RVF?' }, { id: 'at-a-glance', label: 'At a Glance' }] },
  { id: 'single-file', label: 'Single File', icon: '\u25A3',
    children: [{ id: 'segments', label: 'Segment Map' }, { id: 'why-one-file', label: 'Why One File?' }] },
  { id: 'pipeline', label: 'Pipeline', icon: '\u25B6',
    children: [{ id: 'stage-ingest', label: 'Data Ingestion' }, { id: 'stage-process', label: 'Signal Processing' },
      { id: 'stage-detect', label: 'Candidate Detection' }, { id: 'stage-score', label: 'Scoring' }, { id: 'stage-seal', label: 'Witness Sealing' }] },
  { id: 'proof', label: 'Proof', icon: '\u2713',
    children: [{ id: 'witness-chain', label: 'Witness Chain' }, { id: 'reproducible', label: 'Reproducible' },
      { id: 'acceptance', label: 'Acceptance Test' }, { id: 'blind', label: 'Blind Testing' }] },
  { id: 'unique', label: 'Why Unique', icon: '\u2605' },
  { id: 'capabilities', label: 'Views', icon: '\u25CE',
    children: [{ id: 'cap-atlas', label: 'Atlas Explorer' }, { id: 'cap-coherence', label: 'Coherence' },
      { id: 'cap-boundaries', label: 'Boundaries' }, { id: 'cap-memory', label: 'Memory Tiers' },
      { id: 'cap-planets', label: 'Planets' }, { id: 'cap-life', label: 'Life' },
      { id: 'cap-witness', label: 'Witness Chain' }, { id: 'cap-solver', label: 'Solver' },
      { id: 'cap-blind', label: 'Blind Test' }, { id: 'cap-discover', label: 'Discovery' },
      { id: 'cap-dyson', label: 'Dyson Sphere' }, { id: 'cap-status', label: 'Status' }] },
  { id: 'solver', label: 'Solver', icon: '\u2699',
    children: [{ id: 'thompson', label: 'Thompson Sampling' }, { id: 'auto-optimize', label: 'Auto-Optimize' }] },
  { id: 'format', label: 'Format Spec', icon: '\u2630',
    children: [{ id: 'file-header', label: 'File Header' }, { id: 'seg-types', label: 'Segment Types' },
      { id: 'witness-format', label: 'Witness Entry' }, { id: 'dashboard-seg', label: 'Dashboard Segment' }] },
  { id: 'glossary', label: 'Glossary', icon: '\u2261' },
];

export class DocsView {
  private container: HTMLElement | null = null;
  private contentEl: HTMLElement | null = null;
  private navLinks: Map<string, HTMLElement> = new Map();
  private scrollRaf = 0;

  mount(container: HTMLElement): void {
    this.container = container;

    const wrapper = document.createElement('div');
    wrapper.style.cssText = 'display:flex;width:100%;height:100%;overflow:hidden';
    container.appendChild(wrapper);

    // Left nav sidebar
    const nav = this.buildNav();
    wrapper.appendChild(nav);

    // Right content area
    this.contentEl = document.createElement('div');
    this.contentEl.style.cssText = 'flex:1;overflow-y:auto;overflow-x:hidden;scroll-behavior:smooth;-webkit-overflow-scrolling:touch;min-width:0';
    wrapper.appendChild(this.contentEl);

    const inner = document.createElement('div');
    inner.style.cssText = 'max-width:820px;margin:0 auto;padding:28px 32px 100px;line-height:1.7;color:var(--text-secondary);font-size:13px';
    this.contentEl.appendChild(inner);
    inner.innerHTML = this.buildContent();

    // Scroll spy
    this.contentEl.addEventListener('scroll', this.onScroll);
    requestAnimationFrame(() => this.onScroll());
  }

  unmount(): void {
    cancelAnimationFrame(this.scrollRaf);
    this.contentEl?.removeEventListener('scroll', this.onScroll);
    this.navLinks.clear();
    this.contentEl = null;
    this.container = null;
  }

  /* ── Nav sidebar ── */

  private buildNav(): HTMLElement {
    const nav = document.createElement('nav');
    nav.style.cssText = `
      width:220px;min-width:220px;background:var(--bg-panel);border-right:1px solid var(--border);
      overflow-y:auto;overflow-x:hidden;padding:16px 0;display:flex;flex-direction:column;
      -webkit-overflow-scrolling:touch;flex-shrink:0
    `;

    // Title
    const title = document.createElement('div');
    title.style.cssText = 'padding:0 16px 14px;font-size:13px;font-weight:600;color:var(--text-primary);letter-spacing:0.3px;border-bottom:1px solid var(--border);margin-bottom:8px';
    title.textContent = 'Documentation';
    nav.appendChild(title);

    for (const section of SECTIONS) {
      // Parent link
      const link = document.createElement('a');
      link.style.cssText = `
        display:flex;align-items:center;gap:8px;padding:7px 16px;
        font-size:12px;font-weight:600;color:var(--text-secondary);cursor:pointer;
        text-decoration:none;transition:color 0.15s,background 0.15s;border-left:2px solid transparent
      `;
      link.innerHTML = `<span style="font-size:11px;width:16px;text-align:center;opacity:0.6">${section.icon}</span> ${section.label}`;
      link.addEventListener('click', (e) => { e.preventDefault(); this.scrollTo(section.id); });
      link.addEventListener('mouseenter', () => { link.style.color = 'var(--text-primary)'; link.style.background = 'rgba(255,255,255,0.02)'; });
      link.addEventListener('mouseleave', () => {
        if (!link.classList.contains('doc-active')) { link.style.color = 'var(--text-secondary)'; link.style.background = ''; }
      });
      nav.appendChild(link);
      this.navLinks.set(section.id, link);

      // Child links
      if (section.children) {
        for (const child of section.children) {
          const clink = document.createElement('a');
          clink.style.cssText = `
            display:block;padding:4px 16px 4px 40px;font-size:11px;color:var(--text-muted);
            cursor:pointer;text-decoration:none;transition:color 0.15s;border-left:2px solid transparent
          `;
          clink.textContent = child.label;
          clink.addEventListener('click', (e) => { e.preventDefault(); this.scrollTo(child.id); });
          clink.addEventListener('mouseenter', () => { clink.style.color = 'var(--text-secondary)'; });
          clink.addEventListener('mouseleave', () => {
            if (!clink.classList.contains('doc-active')) clink.style.color = 'var(--text-muted)';
          });
          nav.appendChild(clink);
          this.navLinks.set(child.id, clink);
        }
      }
    }

    // Bottom spacer
    const spacer = document.createElement('div');
    spacer.style.cssText = 'flex:1;min-height:20px';
    nav.appendChild(spacer);

    // Footer
    const footer = document.createElement('div');
    footer.style.cssText = 'padding:12px 16px;border-top:1px solid var(--border);font-size:9px;color:var(--text-muted);line-height:1.5';
    footer.innerHTML = 'Built with <span style="color:var(--accent)">RuVector</span><br>Rust + WASM + Three.js';
    nav.appendChild(footer);

    return nav;
  }

  private scrollTo(id: string): void {
    const el = this.contentEl?.querySelector(`#${id}`) as HTMLElement | null;
    if (el && this.contentEl) {
      this.contentEl.scrollTo({ top: el.offsetTop - 20, behavior: 'smooth' });
    }
  }

  /* ── Scroll spy ── */

  private onScroll = (): void => {
    cancelAnimationFrame(this.scrollRaf);
    this.scrollRaf = requestAnimationFrame(() => {
      if (!this.contentEl) return;
      const scrollTop = this.contentEl.scrollTop + 60;

      // Find which section is currently visible
      let activeId = '';
      const allIds = Array.from(this.navLinks.keys());
      for (const id of allIds) {
        const el = this.contentEl.querySelector(`#${id}`) as HTMLElement | null;
        if (el && el.offsetTop <= scrollTop) activeId = id;
      }

      // Update nav highlights
      this.navLinks.forEach((link, id) => {
        const isActive = id === activeId;
        link.classList.toggle('doc-active', isActive);
        // Check if parent or child
        const isParent = SECTIONS.some(s => s.id === id);
        if (isActive) {
          link.style.color = 'var(--accent)';
          link.style.borderLeftColor = 'var(--accent)';
          link.style.background = isParent ? 'rgba(0,229,255,0.06)' : 'rgba(0,229,255,0.03)';
        } else {
          link.style.color = isParent ? 'var(--text-secondary)' : 'var(--text-muted)';
          link.style.borderLeftColor = 'transparent';
          link.style.background = '';
        }
      });
    });
  };

  /* ── Content builder ── */

  private buildContent(): string {
    const S = {
      h1: 'font-size:26px;font-weight:300;color:var(--text-primary);letter-spacing:0.5px;margin-bottom:6px',
      h2: 'font-size:19px;font-weight:600;color:var(--text-primary);margin-top:48px;margin-bottom:10px;padding-bottom:8px;border-bottom:1px solid var(--border)',
      h3: 'font-size:14px;font-weight:600;color:var(--accent);margin-top:28px;margin-bottom:8px',
      p: 'margin-bottom:14px',
      card: 'background:var(--bg-panel);border:1px solid var(--border);border-radius:6px;padding:14px 18px;margin-bottom:12px',
      code: 'font-family:var(--font-mono);font-size:11px;background:var(--bg-surface);border:1px solid var(--border);border-radius:4px;padding:12px 16px;display:block;margin:10px 0 14px;overflow-x:auto;line-height:1.6;color:var(--text-primary)',
      accent: 'color:var(--accent);font-weight:600',
      success: 'color:var(--success);font-weight:600',
      badge: 'display:inline-block;font-size:9px;font-weight:600;padding:2px 8px;border-radius:3px;margin-right:4px',
      inline: 'background:var(--bg-surface);padding:1px 6px;border-radius:3px;font-family:var(--font-mono);font-size:12px',
    };

    return `
<!-- ============ OVERVIEW ============ -->
<div style="${S.h1}" id="overview">Causal Atlas Documentation</div>
<div style="font-size:13px;color:var(--text-muted);margin-bottom:20px">
  A complete guide to the RVF scientific discovery platform.
</div>

<div style="${S.h3}" id="what-is-rvf">What is RVF?</div>
<div style="${S.p}">
  <span style="${S.accent}">RVF (RuVector Format)</span> is a binary container that holds
  an entire scientific discovery pipeline &mdash; raw telescope data, analysis code,
  results, cryptographic proofs, and this interactive dashboard &mdash; in a
  <strong>single, self-contained file</strong>.
</div>
<div style="${S.p}">
  Think of it as a shipping container for science. Anyone who receives the file can
  independently verify every step of the analysis without external tools or databases.
</div>

<div style="${S.h3}" id="at-a-glance">At a Glance</div>
<div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:8px;margin-bottom:16px">
  ${this.statCard('File Format', 'Binary, segmented', S)}
  ${this.statCard('Crypto', 'Ed25519 + SHAKE-256', S)}
  ${this.statCard('Solver', 'WASM + Thompson Sampling', S)}
  ${this.statCard('Dashboard', 'Three.js + D3', S)}
  ${this.statCard('Server', 'Rust / Axum', S)}
  ${this.statCard('Domains', 'Exoplanets, Dyson, Bio', S)}
</div>

<!-- ============ SINGLE FILE ============ -->
<div style="${S.h2}" id="single-file">One File Contains Everything</div>
<div style="${S.p}">
  Traditional scientific data is scattered across files, servers, and packages.
  RVF packs everything into typed <strong>segments</strong> inside one binary file.
</div>

<div style="${S.h3}" id="segments">Segment Map</div>
<div style="${S.card}">
  <div style="font-family:var(--font-mono);font-size:11px;line-height:2.2">
    ${this.segRow('HEADER (64 B)', 'File magic, version, segment count', 'var(--text-muted)')}
    ${this.segRow('DATA_SEG', 'Raw telescope observations (light curves, spectra)', '#FF6B9D')}
    ${this.segRow('KERNEL_SEG', 'Processing algorithms for analysis', '#FFB020')}
    ${this.segRow('EBPF_SEG', 'Fast in-kernel data filtering programs', '#9944FF')}
    ${this.segRow('WASM_SEG', 'Self-learning solver (runs in any browser)', '#2ECC71')}
    ${this.segRow('WITNESS_SEG', 'Cryptographic proof chain (Ed25519 signed)', 'var(--accent)')}
    ${this.segRow('DASHBOARD_SEG', 'This interactive 3D dashboard (HTML/JS/CSS)', '#FF4D4D')}
    ${this.segRow('SIGNATURE', 'Ed25519 signature over all segments', 'var(--text-muted)')}
  </div>
</div>

<div style="${S.h3}" id="why-one-file">Why One File?</div>
<div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:16px">
  <div style="${S.card}padding:10px 14px">
    <div style="font-size:11px;${S.accent}margin-bottom:3px">Portability</div>
    <div style="font-size:11px;line-height:1.5">Email it, USB drive, or static hosting. No server setup needed.</div>
  </div>
  <div style="${S.card}padding:10px 14px">
    <div style="font-size:11px;${S.accent}margin-bottom:3px">Reproducibility</div>
    <div style="font-size:11px;line-height:1.5">Code + data together means anyone can re-run the analysis.</div>
  </div>
  <div style="${S.card}padding:10px 14px">
    <div style="font-size:11px;${S.accent}margin-bottom:3px">Integrity</div>
    <div style="font-size:11px;line-height:1.5">Tampering with any segment breaks the signature chain.</div>
  </div>
  <div style="${S.card}padding:10px 14px">
    <div style="font-size:11px;${S.accent}margin-bottom:3px">Archival</div>
    <div style="font-size:11px;line-height:1.5">One file to store, back up, and cite. No link rot.</div>
  </div>
</div>

<!-- ============ PIPELINE ============ -->
<div style="${S.h2}" id="pipeline">How the Pipeline Works</div>
<div style="${S.p}">
  The pipeline transforms raw observations into verified discoveries through five stages.
  Each stage is recorded in the witness chain for full traceability.
</div>

<div style="${S.h3}" id="stage-ingest">1. Data Ingestion</div>
<div style="${S.p}">
  Raw photometric data (brightness over time) is ingested from telescope archives.
  For exoplanet detection, this means <span style="${S.accent}">light curves</span> &mdash;
  graphs of stellar brightness that dip when a planet transits its star.
</div>

<div style="${S.h3}" id="stage-process">2. Signal Processing</div>
<div style="${S.p}">
  Processing kernels clean the data: removing instrumental noise, correcting for stellar
  variability, and flagging periodic signals. The <span style="${S.accent}">eBPF programs</span>
  accelerate filtering at near-hardware speed.
</div>

<div style="${S.h3}" id="stage-detect">3. Candidate Detection</div>
<div style="${S.p}">
  Cleaned signals are matched against known patterns. For exoplanets: periodic transit-shaped dips.
  For Dyson spheres: anomalous infrared excess. Each candidate gets derived parameters:
</div>
<div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:14px">
  <div style="${S.card}padding:10px 14px">
    <div style="font-size:10px;${S.accent}margin-bottom:3px">Exoplanets</div>
    <div style="font-size:11px">Radius, period, temperature, HZ membership, ESI score</div>
  </div>
  <div style="${S.card}padding:10px 14px">
    <div style="font-size:10px;color:#FFB020;font-weight:600;margin-bottom:3px">Dyson Candidates</div>
    <div style="font-size:11px">IR excess ratio, dimming pattern, partial coverage fraction</div>
  </div>
</div>

<div style="${S.h3}" id="stage-score">4. Scoring &amp; Ranking</div>
<div style="${S.p}">
  Candidates are scored multi-dimensionally. The <span style="${S.accent}">WASM solver</span>
  uses Thompson Sampling to discover which analysis strategies work best for each difficulty
  level, continuously improving accuracy without human tuning.
</div>

<div style="${S.h3}" id="stage-seal">5. Witness Sealing</div>
<div style="${S.p}">
  Every step is recorded in the <span style="${S.accent}">witness chain</span>: a SHAKE-256 hash
  of the data, a timestamp, and an Ed25519 signature. This creates an immutable,
  cryptographically verifiable audit trail.
</div>

<!-- ============ PROOF ============ -->
<div style="${S.h2}" id="proof">How Discoveries Are Proven</div>
<div style="${S.p}">
  <strong>How do you know the results are real?</strong> RVF uses four layers of proof.
</div>

<div style="${S.h3}" id="witness-chain">Layer 1: Cryptographic Witness Chain</div>
<div style="${S.card}">
  <div style="font-size:11px;line-height:1.9;margin-bottom:6px">
    Each processing step writes a witness entry containing:<br>
    &bull; <strong>Step name</strong> &mdash; what operation was performed<br>
    &bull; <strong>Input hash</strong> &mdash; SHAKE-256 of data going in<br>
    &bull; <strong>Output hash</strong> &mdash; SHAKE-256 of data coming out<br>
    &bull; <strong>Parent hash</strong> &mdash; links to previous entry (chain)<br>
    &bull; <strong>Ed25519 signature</strong> &mdash; proves the entry is authentic
  </div>
  <div style="font-size:10px;color:var(--text-muted)">
    Each entry chains to the previous one. Altering any step breaks all subsequent signatures.
  </div>
</div>

<div style="${S.h3}" id="reproducible">Layer 2: Reproducible Computation</div>
<div style="${S.p}">
  The file contains the actual analysis code (WASM + eBPF) alongside raw data.
  Anyone can re-run the pipeline from scratch and verify identical results.
  No "trust us" &mdash; the math is in the file.
</div>

<div style="${S.h3}" id="acceptance">Layer 3: Acceptance Testing</div>
<div style="${S.card}">
  <div style="font-size:11px;line-height:1.9">
    <span style="color:#FF4D4D;font-weight:600">Mode A (Heuristic)</span> &mdash; Can the solver achieve basic accuracy?<br>
    <span style="color:#FFB020;font-weight:600">Mode B (Compiler)</span> &mdash; Accuracy + computational cost reduction?<br>
    <span style="color:#2ECC71;font-weight:600">Mode C (Learned)</span> &mdash; Full: accuracy + cost + robustness + zero violations.
  </div>
  <div style="font-size:10px;color:var(--text-muted);margin-top:6px">
    All three modes must pass. The manifest is itself recorded in the witness chain.
  </div>
</div>

<div style="${S.h3}" id="blind">Layer 4: Blind Testing</div>
<div style="${S.p}">
  The Blind Test page runs the pipeline on unlabeled data, then compares against ground truth.
  This guards against overfitting &mdash; the pipeline must work on data it has never seen.
</div>

<!-- ============ UNIQUE ============ -->
<div style="${S.h2}" id="unique">What Makes This Unique</div>
<div style="display:grid;grid-template-columns:1fr 1fr;gap:10px;margin-bottom:14px">
  ${this.uniqueCard('Self-Contained', 'One file. No cloud, no databases, no external dependencies. The entire pipeline, visualization, and proof chain travel together.', S)}
  ${this.uniqueCard('Cryptographically Verified', 'Every step is hashed and signed. Tampering with one part invalidates the entire chain. Mathematical proof, not just peer review.', S)}
  ${this.uniqueCard('Self-Learning', 'The WASM solver improves over time using Thompson Sampling, discovering which strategies work for different data difficulties.', S)}
  ${this.uniqueCard('Runs Anywhere', 'WASM solver + HTML dashboard + Rust server. No Python, no Jupyter, no conda. Open the file and explore in any modern browser.', S)}
  ${this.uniqueCard('Multi-Domain', 'Transit detection, Dyson sphere search, habitability scoring, biosignature analysis &mdash; all in one causal event graph.', S)}
  ${this.uniqueCard('Interactive 3D', 'Embedded Three.js dashboard: explore the causal atlas as a galaxy, rotate planet systems, visualize Dyson sphere geometry.', S)}
</div>

<!-- ============ CAPABILITIES ============ -->
<div style="${S.h2}" id="capabilities">Dashboard Views</div>
<div style="${S.p}">12 interactive views, each pulling live data from the RVF file.</div>

${this.viewCard('cap-atlas', 'Atlas Explorer', '#/atlas', 'var(--accent)',
  '3D galaxy-style causal event graph. Each star = a causal event. Edges = cause-effect. Configurable arms, density, and sector labels.',
  ['3D OrbitControls', 'Time scale selector', 'Galaxy shape config', 'Star map sectors'])}
${this.viewCard('cap-coherence', 'Coherence Heatmap', '#/coherence', '#FFB020',
  'Color-mapped surface showing data self-consistency across the observation grid. Blue = stable, red = high uncertainty.',
  ['Surface plot', 'Epoch scrubber', 'Partition boundaries'])}
${this.viewCard('cap-boundaries', 'Boundaries', '#/boundaries', '#9944FF',
  'Tracks how data partition boundaries shift as new observations arrive. Alerts when boundaries change rapidly.',
  ['Timeline chart', 'Alert feed', 'Sector detail'])}
${this.viewCard('cap-memory', 'Memory Tiers', '#/memory', '#FF6B9D',
  'Three-tier storage: Small (hot), Medium (warm), Large (cold). Shows utilization, hit rates, and tier migration.',
  ['S/M/L gauges', 'Utilization bars', 'Migration flow'])}
${this.viewCard('cap-planets', 'Planet Candidates', '#/planets', '#2ECC71',
  'Ranked exoplanet candidates with radius, period, temperature, habitable zone status, and Earth Similarity Index.',
  ['Sortable table', 'Light curve plots', 'Score radar'])}
${this.viewCard('cap-life', 'Life Candidates', '#/life', '#2ECC71',
  'Biosignature analysis: atmospheric spectra for O\u2082, CH\u2084, H\u2082O. Multi-dimensional scoring with confound analysis.',
  ['Spectrum plots', 'Molecule heatmap', 'Reaction graph'])}
${this.viewCard('cap-witness', 'Witness Chain', '#/witness', 'var(--accent)',
  'Complete cryptographic audit trail. Every step with timestamp, hashes, and signature verification status.',
  ['Scrolling entries', 'Hash verification', 'Pipeline trace'])}
${this.viewCard('cap-solver', 'RVF Solver', '#/solver', '#FFB020',
  'WASM self-learning solver with Thompson Sampling. 3D landscape shows bandit arm rewards. Configurable training parameters.',
  ['3D landscape', 'Training curves', 'A/B/C acceptance', 'Auto-Optimize'])}
${this.viewCard('cap-blind', 'Blind Test', '#/blind-test', '#FF4D4D',
  'Pipeline on unlabeled data, then compared against ground truth. The gold standard for preventing overfitting.',
  ['Unlabeled processing', 'Ground truth compare', 'Accuracy metrics'])}
${this.viewCard('cap-discover', 'Discovery', '#/discover', '#00E5FF',
  '3D exoplanet systems with host star, orbit, habitable zone. Real KOI parameters. Galaxy background.',
  ['3D planet system', 'Speed/rotate controls', 'ESI comparison'])}
${this.viewCard('cap-dyson', 'Dyson Sphere', '#/dyson', '#9944FF',
  'Dyson swarm detection using Project Hephaistos methodology. IR excess analysis and 3D wireframe visualization.',
  ['3D Dyson wireframe', 'IR excess analysis', 'SED plots'])}
${this.viewCard('cap-status', 'System Status', '#/status', '#8B949E',
  'RVF file health, segment sizes, memory tier utilization, pipeline stage indicators, and live witness log.',
  ['Segment breakdown', 'Tier gauges', 'Witness log feed'])}

<!-- ============ SOLVER ============ -->
<div style="${S.h2}" id="solver">The Self-Learning Solver</div>
<div style="${S.p}">
  The solver is a <span style="${S.accent}">WebAssembly module</span> compiled from Rust.
  It runs entirely in your browser using <strong>Thompson Sampling</strong>.
</div>

<div style="${S.h3}" id="thompson">How Thompson Sampling Works</div>
<div style="${S.p}">
  Imagine 8 different analysis strategies ("arms"). You don't know which works best.
  Thompson Sampling maintains a Beta distribution for each arm's success rate,
  samples from these on each attempt, and picks the highest sample. This balances:
</div>
<div style="${S.card}">
  <div style="font-size:12px;line-height:1.8">
    <span style="${S.accent}">Exploration</span> &mdash; Trying uncertain arms to gather data<br>
    <span style="${S.success}">Exploitation</span> &mdash; Using known-good arms to maximize results
  </div>
  <div style="font-size:10px;color:var(--text-muted);margin-top:6px">
    Over time, the solver converges on optimal strategies per difficulty level.
    The 3D landscape visually shows which arms have the highest rewards.
  </div>
</div>

<div style="${S.h3}" id="auto-optimize">Auto-Optimize</div>
<div style="${S.p}">
  The <span style="${S.success}">Auto-Optimize</span> button trains in batches of 3 rounds,
  tests acceptance after each batch, and stops when all three modes pass (max 30 rounds).
  If accuracy is below 60%, it automatically increases training intensity.
</div>

<!-- ============ FORMAT ============ -->
<div style="${S.h2}" id="format">RVF File Format Reference</div>

<div style="${S.h3}" id="file-header">File Header (64 bytes)</div>
<pre style="${S.code}">Offset  Size  Field
0x00    4     Magic: 0x52564631 ("RVF1")
0x04    2     Format version (currently 1)
0x06    2     Flags (bit 0 = signed, bit 1 = compressed)
0x08    8     Total file size
0x10    4     Segment count
0x14    4     Reserved
0x18    32    SHAKE-256 hash of all segments
0x38    8     Creation timestamp (Unix epoch)</pre>

<div style="${S.h3}" id="seg-types">Segment Types</div>
<div style="overflow-x:auto;margin-bottom:14px">
  <table style="width:100%;font-size:11px;font-family:var(--font-mono);border-collapse:collapse">
    <tr style="border-bottom:1px solid var(--border)">
      <th style="padding:6px 8px;text-align:left;color:var(--text-muted);font-weight:500;width:50px">ID</th>
      <th style="padding:6px 8px;text-align:left;color:var(--text-muted);font-weight:500;width:110px">Name</th>
      <th style="padding:6px 8px;text-align:left;color:var(--text-muted);font-weight:500">Purpose</th>
    </tr>
    ${this.tableRow('0x01', 'DATA', 'Raw observations (light curves, spectra)')}
    ${this.tableRow('0x02', 'KERNEL', 'Processing algorithms')}
    ${this.tableRow('0x03', 'RESULT', 'Computed results and derived parameters')}
    ${this.tableRow('0x04', 'WITNESS', 'Cryptographic audit trail')}
    ${this.tableRow('0x05', 'SIGNATURE', 'Ed25519 digital signature')}
    ${this.tableRow('0x06', 'INDEX', 'Fast lookup table for segments')}
    ${this.tableRow('0x0F', 'EBPF', 'eBPF bytecode for in-kernel filtering')}
    ${this.tableRow('0x10', 'WASM', 'WebAssembly solver module')}
    ${this.tableRow('0x11', 'DASHBOARD', 'Embedded web dashboard (HTML/JS/CSS)')}
  </table>
</div>

<div style="${S.h3}" id="witness-format">Witness Entry Format</div>
<pre style="${S.code}">struct WitnessEntry {
    step_name:   String,      // "transit_detection"
    timestamp:   u64,         // Unix epoch nanoseconds
    input_hash:  [u8; 32],    // SHAKE-256 of input
    output_hash: [u8; 32],    // SHAKE-256 of output
    parent_hash: [u8; 32],    // Previous entry hash (chain)
    signature:   [u8; 64],    // Ed25519 signature
}</pre>

<div style="${S.h3}" id="dashboard-seg">Dashboard Segment</div>
<pre style="${S.code}">DashboardHeader (64 bytes):
    magic:       0x5256_4442  // "RVDB"
    version:     u16
    framework:   u8           // 0=threejs, 1=react
    compression: u8           // 0=none, 1=gzip, 2=brotli
    bundle_size: u64
    file_count:  u32
    hash:        [u8; 32]     // SHAKE-256 of bundle

Payload: [file_table] [file_data...]</pre>

<!-- ============ GLOSSARY ============ -->
<div style="${S.h2}" id="glossary">Glossary</div>
<div style="display:grid;grid-template-columns:130px 1fr;gap:1px 14px;font-size:12px;line-height:2">
  ${this.glossaryRow('RVF', 'RuVector Format &mdash; the binary container')}
  ${this.glossaryRow('Segment', 'A typed block of data inside an RVF file')}
  ${this.glossaryRow('Witness Chain', 'Linked list of signed hash entries proving integrity')}
  ${this.glossaryRow('SHAKE-256', 'Cryptographic hash function (variable output)')}
  ${this.glossaryRow('Ed25519', 'Digital signature algorithm for witness entries')}
  ${this.glossaryRow('KOI', 'Kepler Object of Interest &mdash; exoplanet candidate')}
  ${this.glossaryRow('ESI', 'Earth Similarity Index (0-1, higher = more Earth-like)')}
  ${this.glossaryRow('Transit', 'Planet passing in front of its star, causing a brightness dip')}
  ${this.glossaryRow('Light Curve', 'Graph of stellar brightness over time')}
  ${this.glossaryRow('Habitable Zone', 'Orbital region where liquid water could exist')}
  ${this.glossaryRow('Thompson Samp.', 'Bandit algorithm balancing exploration vs exploitation')}
  ${this.glossaryRow('eBPF', 'Extended Berkeley Packet Filter &mdash; fast kernel programs')}
  ${this.glossaryRow('WASM', 'WebAssembly &mdash; portable code that runs in browsers')}
  ${this.glossaryRow('Dyson Sphere', 'Hypothetical megastructure around a star for energy')}
  ${this.glossaryRow('IR Excess', 'More infrared than expected &mdash; possible artificial origin')}
  ${this.glossaryRow('SED', 'Spectral Energy Distribution &mdash; brightness vs wavelength')}
  ${this.glossaryRow('Coherence', 'Self-consistency measure of data in a region')}
  ${this.glossaryRow('Acceptance', 'Three-mode validation (A/B/C) of solver quality')}
  ${this.glossaryRow('Blind Test', 'Evaluation on unlabeled data to prevent overfitting')}
</div>

<div style="margin-top:48px;padding-top:16px;border-top:1px solid var(--border);font-size:11px;color:var(--text-muted);text-align:center">
  Everything in this dashboard was served from a single <code style="${S.inline}">.rvf</code> file.
</div>
    `;
  }

  /* ── Template helpers ── */

  private statCard(label: string, value: string, S: Record<string, string>): string {
    return `<div style="${S.card}padding:10px 12px;text-align:center">
      <div style="font-size:9px;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.4px;margin-bottom:2px">${label}</div>
      <div style="font-size:12px;font-weight:600;color:var(--accent);font-family:var(--font-mono)">${value}</div>
    </div>`;
  }

  private segRow(name: string, desc: string, color: string): string {
    return `<div style="display:flex;align-items:center;gap:10px"><span style="color:${color};min-width:160px;font-weight:600">${name}</span><span style="color:var(--text-secondary);font-weight:400">${desc}</span></div>`;
  }

  private uniqueCard(title: string, desc: string, S: Record<string, string>): string {
    return `<div style="${S.card}"><div style="font-size:12px;${S.accent}margin-bottom:4px">${title}</div><div style="font-size:11px;line-height:1.5">${desc}</div></div>`;
  }

  private viewCard(id: string, title: string, route: string, color: string, desc: string, features: string[]): string {
    const badges = features.map(f => `<span style="font-size:9px;padding:2px 6px;border-radius:3px;background:rgba(0,229,255,0.06);border:1px solid rgba(0,229,255,0.1);color:var(--accent)">${f}</span>`).join('');
    return `<div id="${id}" style="background:var(--bg-panel);border:1px solid var(--border);border-radius:6px;padding:12px 16px;margin-bottom:8px;border-left:3px solid ${color}">
      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:4px">
        <span style="font-size:12px;font-weight:600;color:var(--text-primary)">${title}</span>
        <a href="${route}" style="font-size:10px;color:${color};font-family:var(--font-mono);text-decoration:none">${route}</a>
      </div>
      <div style="font-size:11px;line-height:1.5;margin-bottom:6px">${desc}</div>
      <div style="display:flex;flex-wrap:wrap;gap:3px">${badges}</div>
    </div>`;
  }

  private tableRow(id: string, name: string, purpose: string): string {
    return `<tr style="border-bottom:1px solid var(--border-subtle)"><td style="padding:5px 8px;color:var(--accent)">${id}</td><td style="padding:5px 8px;color:var(--text-primary)">${name}</td><td style="padding:5px 8px">${purpose}</td></tr>`;
  }

  private glossaryRow(term: string, def: string): string {
    return `<span style="color:var(--accent);font-weight:600">${term}</span><span>${def}</span>`;
  }
}
