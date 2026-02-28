/**
 * DownloadView — Download page for RVF executables and packages.
 *
 * Provides download links for:
 * - Windows (.exe)
 * - macOS (.dmg)
 * - Linux (.tar.gz)
 * - npm packages
 * - WASM module
 *
 * Download URLs point to Google Cloud Storage (placeholder paths).
 */

const VERSION = '2.0.0';
const BASE_URL = 'https://storage.googleapis.com/ruvector-releases';

interface DownloadItem {
  platform: string;
  icon: string;
  file: string;
  size: string;
  ext: string;
  desc: string;
}

const DOWNLOADS: DownloadItem[] = [
  {
    platform: 'Windows',
    icon: '<svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3" y="3" width="8" height="8"/><rect x="13" y="3" width="8" height="8"/><rect x="3" y="13" width="8" height="8"/><rect x="13" y="13" width="8" height="8"/></svg>',
    file: `ruvector-${VERSION}-x64.exe`,
    size: '~12 MB',
    ext: '.exe',
    desc: 'Windows 10/11 (x64) installer with bundled WASM runtime',
  },
  {
    platform: 'macOS',
    icon: '<svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2z"/><path d="M15 8.5c0-1-0.67-2.5-2-2.5S11 7.5 11 8.5c0 1.5 1 2 1 3.5s-1 2-1 3.5c0 1 0.67 2.5 2 2.5s2-1.5 2-2.5c0-1.5-1-2-1-3.5s1-2 1-3.5z"/></svg>',
    file: `RuVector-${VERSION}.dmg`,
    size: '~14 MB',
    ext: '.dmg',
    desc: 'macOS 12+ (Apple Silicon & Intel) disk image',
  },
  {
    platform: 'Linux',
    icon: '<svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="12" cy="12" r="9"/><path d="M12 3v18M3 12h18"/><circle cx="12" cy="8" r="1.5"/></svg>',
    file: `ruvector-${VERSION}-linux-x64.tar.gz`,
    size: '~10 MB',
    ext: '.tar.gz',
    desc: 'Linux (x86_64) tarball — Ubuntu 20+, Debian 11+, Fedora 36+',
  },
];

export class DownloadView {
  private container: HTMLElement | null = null;

  mount(container: HTMLElement): void {
    this.container = container;

    const wrapper = document.createElement('div');
    wrapper.style.cssText = 'max-width:960px;margin:0 auto;padding:32px 24px;overflow-y:auto;height:100%';
    container.appendChild(wrapper);

    // Hero
    const hero = document.createElement('div');
    hero.style.cssText = 'text-align:center;margin-bottom:40px';
    hero.innerHTML = `
      <div style="display:inline-flex;align-items:center;gap:12px;margin-bottom:16px">
        <svg viewBox="0 0 24 24" width="40" height="40" fill="none" stroke="#00E5FF" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="6"/><ellipse cx="12" cy="12" rx="11" ry="4" transform="rotate(-20 12 12)"/>
          <circle cx="12" cy="12" r="1.5" fill="#00E5FF" stroke="none"/>
        </svg>
        <span style="font-size:28px;font-weight:300;color:var(--text-primary);letter-spacing:2px">RuVector</span>
      </div>
      <div style="font-size:14px;color:var(--text-secondary);line-height:1.6;max-width:600px;margin:0 auto">
        Download the Causal Atlas runtime — a single binary that reads <code style="color:var(--accent);font-size:12px">.rvf</code> files,
        runs the WASM solver, serves the Three.js dashboard, and verifies the Ed25519 witness chain.
      </div>
      <div style="margin-top:12px;display:flex;gap:8px;justify-content:center;flex-wrap:wrap">
        <span style="font-size:10px;padding:3px 8px;border-radius:4px;background:rgba(0,229,255,0.08);border:1px solid rgba(0,229,255,0.15);color:#00E5FF">v${VERSION}</span>
        <span style="font-size:10px;padding:3px 8px;border-radius:4px;background:rgba(46,204,113,0.08);border:1px solid rgba(46,204,113,0.15);color:#2ECC71">Stable</span>
        <span style="font-size:10px;padding:3px 8px;border-radius:4px;background:rgba(255,176,32,0.08);border:1px solid rgba(255,176,32,0.15);color:#FFB020">ADR-040</span>
      </div>
    `;
    wrapper.appendChild(hero);

    // Download cards
    const grid = document.createElement('div');
    grid.style.cssText = 'display:grid;grid-template-columns:repeat(auto-fit, minmax(280px, 1fr));gap:16px;margin-bottom:40px';
    wrapper.appendChild(grid);

    for (const dl of DOWNLOADS) {
      const card = document.createElement('div');
      card.style.cssText = `
        background:var(--bg-surface);border:1px solid var(--border);border-radius:8px;
        padding:20px;display:flex;flex-direction:column;gap:12px;
        transition:border-color 0.2s,transform 0.2s;cursor:pointer;
      `;
      card.addEventListener('mouseenter', () => {
        card.style.borderColor = 'rgba(0,229,255,0.3)';
        card.style.transform = 'translateY(-2px)';
      });
      card.addEventListener('mouseleave', () => {
        card.style.borderColor = 'var(--border)';
        card.style.transform = '';
      });

      card.innerHTML = `
        <div style="display:flex;align-items:center;gap:12px">
          <div style="color:#00E5FF">${dl.icon}</div>
          <div>
            <div style="font-size:15px;font-weight:600;color:var(--text-primary)">${dl.platform}</div>
            <div style="font-size:10px;color:var(--text-muted)">${dl.size}</div>
          </div>
        </div>
        <div style="font-size:11px;color:var(--text-secondary);line-height:1.5">${dl.desc}</div>
        <div style="font-family:var(--font-mono);font-size:10px;color:var(--text-muted);padding:6px 8px;background:rgba(0,0,0,0.3);border-radius:4px;word-break:break-all">${dl.file}</div>
        <a href="${BASE_URL}/v${VERSION}/${dl.file}" style="
          display:flex;align-items:center;justify-content:center;gap:6px;
          padding:8px 16px;border-radius:6px;text-decoration:none;
          background:rgba(0,229,255,0.1);border:1px solid rgba(0,229,255,0.25);
          color:#00E5FF;font-size:12px;font-weight:600;transition:background 0.15s;
        " onmouseenter="this.style.background='rgba(0,229,255,0.2)'" onmouseleave="this.style.background='rgba(0,229,255,0.1)'">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
          Download ${dl.ext}
        </a>
      `;
      grid.appendChild(card);
    }

    // npm / WASM section
    const altSection = document.createElement('div');
    altSection.style.cssText = 'margin-bottom:40px';
    altSection.innerHTML = `
      <div style="font-size:13px;font-weight:600;color:var(--text-primary);margin-bottom:16px;display:flex;align-items:center;gap:8px">
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18M9 21V9"/></svg>
        npm Packages &amp; WASM Module
      </div>
      <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
        <div style="background:var(--bg-surface);border:1px solid var(--border);border-radius:6px;padding:14px">
          <div style="font-size:11px;font-weight:600;color:var(--text-primary);margin-bottom:6px">rvf-solver (npm)</div>
          <code style="display:block;font-size:10px;color:var(--accent);background:rgba(0,0,0,0.3);padding:8px;border-radius:4px;margin-bottom:6px">npm install @ruvector/rvf-solver</code>
          <div style="font-size:10px;color:var(--text-muted);line-height:1.4">NAPI-RS native bindings for Node.js — includes solver, witness chain, and policy kernel.</div>
        </div>
        <div style="background:var(--bg-surface);border:1px solid var(--border);border-radius:6px;padding:14px">
          <div style="font-size:11px;font-weight:600;color:var(--text-primary);margin-bottom:6px">rvf-solver-wasm (npm)</div>
          <code style="display:block;font-size:10px;color:var(--accent);background:rgba(0,0,0,0.3);padding:8px;border-radius:4px;margin-bottom:6px">npm install @ruvector/rvf-solver-wasm</code>
          <div style="font-size:10px;color:var(--text-muted);line-height:1.4">Browser WASM module — same solver running in this dashboard. No native dependencies.</div>
        </div>
        <div style="background:var(--bg-surface);border:1px solid var(--border);border-radius:6px;padding:14px">
          <div style="font-size:11px;font-weight:600;color:var(--text-primary);margin-bottom:6px">Standalone WASM</div>
          <code style="display:block;font-size:10px;color:var(--accent);background:rgba(0,0,0,0.3);padding:8px;border-radius:4px;margin-bottom:6px">curl -O ${BASE_URL}/v${VERSION}/rvf_solver_wasm.wasm</code>
          <div style="font-size:10px;color:var(--text-muted);line-height:1.4">Raw <code>.wasm</code> binary (172 KB). Load via WebAssembly.instantiate() — no wasm-bindgen needed.</div>
        </div>
        <div style="background:var(--bg-surface);border:1px solid var(--border);border-radius:6px;padding:14px">
          <div style="font-size:11px;font-weight:600;color:var(--text-primary);margin-bottom:6px">Cargo Crate</div>
          <code style="display:block;font-size:10px;color:var(--accent);background:rgba(0,0,0,0.3);padding:8px;border-radius:4px;margin-bottom:6px">cargo add rvf-runtime rvf-types rvf-crypto</code>
          <div style="font-size:10px;color:var(--text-muted);line-height:1.4">Rust workspace crates for embedding RVF files in your own applications.</div>
        </div>
      </div>
    `;
    wrapper.appendChild(altSection);

    // Quick Start section
    const quickstart = document.createElement('div');
    quickstart.style.cssText = 'margin-bottom:40px';
    quickstart.innerHTML = `
      <div style="font-size:13px;font-weight:600;color:var(--text-primary);margin-bottom:16px;display:flex;align-items:center;gap:8px">
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.5"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
        Quick Start
      </div>
      <div style="background:var(--bg-surface);border:1px solid var(--border);border-radius:8px;padding:20px">
        <div style="display:grid;gap:16px">
          ${this.step(1, 'Download', 'Download the installer for your platform above and run it.')}
          ${this.step(2, 'Open an RVF file', `
            <code style="display:block;font-size:11px;color:var(--accent);background:rgba(0,0,0,0.3);padding:8px;border-radius:4px;margin-top:4px">ruvector open causal_atlas.rvf</code>
            <div style="font-size:10px;color:var(--text-muted);margin-top:4px">This starts the local server and opens the dashboard in your browser.</div>
          `)}
          ${this.step(3, 'Train the solver', 'Navigate to the Solver page and click Train or Auto-Optimize. The WASM solver learns in real time inside your browser.')}
          ${this.step(4, 'Run acceptance test', 'Click Acceptance to verify the solver passes the three-mode acceptance test (A/B/C). All results are recorded in the Ed25519 witness chain.')}
          ${this.step(5, 'Explore discoveries', `Navigate to Planets, Life, and Discover pages to explore candidate detections. Each candidate includes a full causal trace and witness proof.`)}
        </div>
      </div>
    `;
    wrapper.appendChild(quickstart);

    // System requirements
    const reqs = document.createElement('div');
    reqs.innerHTML = `
      <div style="font-size:13px;font-weight:600;color:var(--text-primary);margin-bottom:16px;display:flex;align-items:center;gap:8px">
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="4" y="4" width="16" height="16" rx="2"/><line x1="4" y1="9" x2="20" y2="9"/><circle cx="8" cy="6.5" r="0.5" fill="currentColor" stroke="none"/></svg>
        System Requirements
      </div>
      <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:12px;font-size:11px">
        <div style="background:var(--bg-surface);border:1px solid var(--border);border-radius:6px;padding:12px">
          <div style="font-weight:600;color:var(--text-primary);margin-bottom:6px">Windows</div>
          <div style="color:var(--text-muted);line-height:1.5">Windows 10 (1903+)<br>x64 processor<br>4 GB RAM<br>100 MB disk</div>
        </div>
        <div style="background:var(--bg-surface);border:1px solid var(--border);border-radius:6px;padding:12px">
          <div style="font-weight:600;color:var(--text-primary);margin-bottom:6px">macOS</div>
          <div style="color:var(--text-muted);line-height:1.5">macOS 12 Monterey+<br>Apple Silicon or Intel<br>4 GB RAM<br>100 MB disk</div>
        </div>
        <div style="background:var(--bg-surface);border:1px solid var(--border);border-radius:6px;padding:12px">
          <div style="font-weight:600;color:var(--text-primary);margin-bottom:6px">Linux</div>
          <div style="color:var(--text-muted);line-height:1.5">glibc 2.31+<br>x86_64 processor<br>4 GB RAM<br>100 MB disk</div>
        </div>
      </div>
      <div style="font-size:9px;color:var(--text-muted);margin-top:12px;text-align:center">
        Binaries are hosted on Google Cloud Storage. All downloads include Ed25519 signatures for verification.
      </div>
    `;
    wrapper.appendChild(reqs);
  }

  private step(n: number, title: string, detail: string): string {
    return `
      <div style="display:flex;gap:12px;align-items:flex-start">
        <div style="min-width:24px;height:24px;border-radius:50%;background:rgba(0,229,255,0.1);border:1px solid rgba(0,229,255,0.25);display:flex;align-items:center;justify-content:center;font-size:11px;font-weight:700;color:#00E5FF">${n}</div>
        <div style="flex:1">
          <div style="font-size:12px;font-weight:600;color:var(--text-primary);margin-bottom:2px">${title}</div>
          <div style="font-size:11px;color:var(--text-secondary);line-height:1.5">${detail}</div>
        </div>
      </div>
    `;
  }

  unmount(): void {
    this.container = null;
  }
}
