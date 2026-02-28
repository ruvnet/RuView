export interface SidebarItem {
  id: string;
  name: string;
  score?: number;
}

type SelectCallback = (id: string) => void;

export class Sidebar {
  private root: HTMLElement;
  private listEl: HTMLElement;
  private filterInput: HTMLInputElement;
  private items: SidebarItem[] = [];
  private activeId: string | null = null;
  private selectCallback: SelectCallback | null = null;
  private customFilter: ((item: SidebarItem) => boolean) | null = null;

  constructor(container: HTMLElement) {
    this.root = document.createElement('div');
    this.root.className = 'sidebar';

    // Filter
    this.filterInput = document.createElement('input');
    this.filterInput.type = 'text';
    this.filterInput.className = 'sidebar-search';
    this.filterInput.placeholder = 'Filter...';
    this.filterInput.addEventListener('input', () => this.applyFilter());
    this.root.appendChild(this.filterInput);

    // List
    this.listEl = document.createElement('div');
    this.listEl.className = 'sidebar-list';
    this.root.appendChild(this.listEl);

    container.appendChild(this.root);
  }

  setItems(items: SidebarItem[]): void {
    this.items = items;
    this.applyFilter();
  }

  onSelect(callback: SelectCallback): void {
    this.selectCallback = callback;
  }

  setFilter(filterFn: (item: SidebarItem) => boolean): void {
    this.customFilter = filterFn;
    this.applyFilter();
  }

  private applyFilter(): void {
    const query = this.filterInput.value.toLowerCase().trim();
    const filtered = this.items.filter((item) => {
      if (this.customFilter && !this.customFilter(item)) return false;
      if (query && !item.name.toLowerCase().includes(query)) return false;
      return true;
    });
    this.render(filtered);
  }

  private render(filtered: SidebarItem[]): void {
    this.listEl.innerHTML = '';
    for (const item of filtered) {
      const el = document.createElement('div');
      el.className = 'sidebar-item';
      if (item.id === this.activeId) el.classList.add('selected');

      const nameSpan = document.createElement('span');
      nameSpan.className = 'sidebar-item-label';
      nameSpan.textContent = item.name;
      el.appendChild(nameSpan);

      if (item.score !== undefined) {
        const scoreSpan = document.createElement('span');
        scoreSpan.className = 'sidebar-item-secondary';
        scoreSpan.textContent = `Score: ${item.score.toFixed(2)}`;
        el.appendChild(scoreSpan);
      }

      el.addEventListener('click', () => {
        this.activeId = item.id;
        this.applyFilter();
        this.selectCallback?.(item.id);
      });

      this.listEl.appendChild(el);
    }
  }

  destroy(): void {
    this.root.remove();
  }
}
