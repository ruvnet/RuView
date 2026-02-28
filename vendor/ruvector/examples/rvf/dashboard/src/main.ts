import { AtlasExplorer } from './views/AtlasExplorer';
import { CoherenceHeatmap } from './views/CoherenceHeatmap';
import { BoundariesView } from './views/BoundariesView';
import { MemoryView } from './views/MemoryView';
import { PlanetDashboard } from './views/PlanetDashboard';
import { LifeDashboard } from './views/LifeDashboard';
import { WitnessView } from './views/WitnessView';
import { SolverDashboard } from './views/SolverDashboard';
import { StatusDashboard } from './views/StatusDashboard';
import { BlindTestView } from './views/BlindTestView';
import { DiscoveryView } from './views/DiscoveryView';
import { DysonSphereView } from './views/DysonSphereView';
import { DocsView } from './views/DocsView';
import { DownloadView } from './views/DownloadView';
import { connect, disconnect } from './ws';
import { fetchStatus } from './api';
import './styles/main.css';

type ViewClass = { new (): { mount(el: HTMLElement): void; unmount(): void } };

const routes: Record<string, ViewClass> = {
  '#/atlas': AtlasExplorer,
  '#/coherence': CoherenceHeatmap,
  '#/boundaries': BoundariesView,
  '#/memory': MemoryView,
  '#/planets': PlanetDashboard,
  '#/life': LifeDashboard,
  '#/witness': WitnessView,
  '#/solver': SolverDashboard,
  '#/blind-test': BlindTestView,
  '#/discover': DiscoveryView,
  '#/dyson': DysonSphereView,
  '#/status': StatusDashboard,
  '#/download': DownloadView,
  '#/docs': DocsView,
};

let currentView: { unmount(): void } | null = null;

function getAppContainer(): HTMLElement {
  const el = document.getElementById('app');
  if (!el) throw new Error('Missing #app container');
  return el;
}

function updateActiveLink(): void {
  const hash = location.hash || '#/atlas';
  document.querySelectorAll('#nav-rail a').forEach((a) => {
    const anchor = a as HTMLAnchorElement;
    anchor.classList.toggle('active', anchor.getAttribute('href') === hash);
  });
}

function navigateTo(hash: string): void {
  const container = getAppContainer();

  if (currentView) {
    currentView.unmount();
    currentView = null;
  }
  container.innerHTML = '';

  const ViewCtor = routes[hash] || routes['#/atlas'];
  const view = new ViewCtor();
  view.mount(container);
  currentView = view;

  updateActiveLink();
}

async function updateRootHash(): Promise<void> {
  const hashEl = document.getElementById('root-hash');
  const dotEl = document.querySelector('#top-bar .dot') as HTMLElement | null;
  const statusEl = document.getElementById('pipeline-status');
  if (!hashEl) return;

  try {
    const status = await fetchStatus();
    const h = ((status.file_size * 0x5DEECE66 + status.segments) >>> 0).toString(16).padStart(8, '0');
    hashEl.textContent = `0x${h.substring(0, 4)}...${h.substring(4, 8)}`;
    if (dotEl) dotEl.style.background = '#2ECC71';
    if (statusEl) {
      statusEl.textContent = 'LIVE';
      statusEl.style.color = '#2ECC71';
    }
  } catch {
    hashEl.textContent = '0x----...----';
    if (dotEl) dotEl.style.background = '#FF4D4D';
    if (statusEl) {
      statusEl.textContent = 'OFFLINE';
      statusEl.style.color = '#FF4D4D';
    }
  }
}

function init(): void {
  connect();

  const initialHash = location.hash || '#/atlas';
  if (!location.hash) {
    location.hash = '#/atlas';
  }
  navigateTo(initialHash);

  window.addEventListener('hashchange', () => {
    navigateTo(location.hash);
  });

  window.addEventListener('beforeunload', () => {
    disconnect();
  });

  // Update root hash display
  updateRootHash();
  setInterval(updateRootHash, 10000);
}

document.addEventListener('DOMContentLoaded', init);
