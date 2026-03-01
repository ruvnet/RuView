export interface WitnessLogEntry {
  timestamp: string;
  type: string;
  action: string;
  hash: string;
}

const BADGE_CLASS: Record<string, string> = {
  commit: 'witness-badge-commit',
  verify: 'witness-badge-verify',
  seal: 'witness-badge-seal',
  merge: 'witness-badge-merge',
};

export class WitnessLog {
  private root: HTMLElement;
  private listEl: HTMLElement;
  private autoScroll = true;

  constructor(container: HTMLElement) {
    this.root = document.createElement('div');
    this.root.className = 'witness-log';

    const header = document.createElement('div');
    header.className = 'witness-log-header';
    header.textContent = 'Witness Log';
    this.root.appendChild(header);

    this.listEl = document.createElement('div');
    this.listEl.className = 'witness-log-list';

    this.listEl.addEventListener('scroll', () => {
      const { scrollTop, scrollHeight, clientHeight } = this.listEl;
      this.autoScroll = scrollTop + clientHeight >= scrollHeight - 20;
    });

    this.root.appendChild(this.listEl);
    container.appendChild(this.root);
  }

  addEntry(entry: WitnessLogEntry): void {
    const el = document.createElement('div');
    el.className = 'witness-log-entry';

    const ts = document.createElement('span');
    ts.className = 'witness-ts';
    ts.textContent = entry.timestamp;
    el.appendChild(ts);

    const typeBadge = document.createElement('span');
    const badgeCls = BADGE_CLASS[entry.type.toLowerCase()] ?? 'witness-badge-commit';
    typeBadge.className = `witness-badge ${badgeCls}`;
    typeBadge.textContent = entry.type;
    el.appendChild(typeBadge);

    const action = document.createElement('span');
    action.className = 'witness-step';
    action.textContent = entry.action;
    el.appendChild(action);

    const hash = document.createElement('span');
    hash.className = 'witness-hash';
    hash.textContent = entry.hash.substring(0, 12);
    hash.title = entry.hash;
    el.appendChild(hash);

    this.listEl.appendChild(el);

    if (this.autoScroll) {
      this.listEl.scrollTop = this.listEl.scrollHeight;
    }
  }

  clear(): void {
    this.listEl.innerHTML = '';
  }

  destroy(): void {
    this.root.remove();
  }
}
