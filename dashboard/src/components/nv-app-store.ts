/* App Store — catalog of every WASM edge module + simulator app.
 *
 * Mirrors `wifi-densepose-wasm-edge`'s 60+ hot-loadable algorithms and
 * the `nvsim` simulator. Each card is filterable by category, fuzzy
 * name search, and maturity (available / beta / research). A toggle on
 * each card flips activation in the live session — that drives the
 * dashboard's event log when running. WS transport (future) pushes the
 * activation set to the connected ESP32 mesh.
 *
 * ADR-092 §18.
 */

import { LitElement, html, css } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import { signal, effect } from '@preact/signals-core';
import {
  APPS, CATEGORIES, defaultActivations, fuzzyMatch,
  type AppCategory, type AppManifest, type AppActivation,
} from '../store/apps';
import { kvGet, kvSet } from '../store/persistence';
import { pushLog } from '../store/appStore';

const activations = signal<AppActivation[]>(defaultActivations());
const query = signal<string>('');
const activeCat = signal<AppCategory | 'all'>('all');
const statusFilter = signal<'all' | 'available' | 'beta' | 'research'>('all');

(async () => {
  const saved = await kvGet<AppActivation[]>('app-activations');
  if (saved) activations.value = saved;
})();

effect(() => {
  // Persist activations on change (post-load).
  const v = activations.value;
  if (v.length > 0) void kvSet('app-activations', v);
});

@customElement('nv-app-store')
export class NvAppStore extends LitElement {
  @state() private renderTick = 0;

  static styles = css`
    :host {
      display: block;
      height: 100%;
      overflow-y: auto;
      background: radial-gradient(ellipse at 50% 30%, var(--bg-2) 0%, var(--bg-0) 70%);
      padding: 24px;
    }
    .head {
      display: flex; align-items: center; gap: 16px;
      margin-bottom: 18px;
      flex-wrap: wrap;
    }
    .ttl {
      font-size: 22px; font-weight: 700; letter-spacing: -0.02em;
      color: var(--ink);
      flex: 1; min-width: 200px;
    }
    .ttl small {
      font-size: 12.5px; font-weight: 400;
      color: var(--ink-3); margin-left: 8px;
    }
    .search {
      width: 320px; max-width: 100%;
      padding: 8px 12px;
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: 8px;
      font-family: var(--mono);
      font-size: 12.5px;
      color: var(--ink); outline: none;
    }
    .search:focus { border-color: var(--accent); }
    .filters {
      display: flex; flex-wrap: wrap; gap: 6px;
      margin-bottom: 18px;
    }
    .chip {
      padding: 4px 10px;
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: 999px;
      font-size: 11.5px; color: var(--ink-3);
      cursor: pointer;
      font-family: var(--mono);
      display: inline-flex; align-items: center; gap: 4px;
    }
    .chip:hover { color: var(--ink); border-color: var(--line-2); }
    .chip.on { background: var(--bg-3); border-color: var(--accent); color: var(--ink); }
    .chip .swatch {
      width: 7px; height: 7px; border-radius: 50%;
    }
    .chip .count { color: var(--ink-3); font-size: 10px; }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
      gap: 12px;
    }
    .card {
      background: var(--bg-2);
      border: 1px solid var(--line);
      border-radius: var(--radius);
      padding: 12px 14px;
      display: flex; flex-direction: column; gap: 6px;
      transition: border-color 0.15s, transform 0.15s;
      position: relative;
    }
    .card:hover { border-color: var(--line-2); transform: translateY(-1px); }
    .card.active {
      border-color: oklch(0.78 0.14 145 / 0.7);
      background: linear-gradient(180deg, var(--bg-2) 0%, oklch(0.78 0.14 145 / 0.04) 100%);
    }
    .card-h {
      display: flex; align-items: flex-start; gap: 8px;
      margin-bottom: 2px;
    }
    .card-h .name {
      font-size: 13.5px; font-weight: 600; color: var(--ink);
      flex: 1; line-height: 1.3;
    }
    .card-h .swatch {
      width: 10px; height: 10px; border-radius: 50%;
      flex-shrink: 0; margin-top: 4px;
    }
    .summary {
      font-size: 12px; color: var(--ink-2); line-height: 1.45;
      flex: 1;
    }
    .meta {
      display: flex; flex-wrap: wrap; gap: 4px; margin-top: 6px;
      font-family: var(--mono); font-size: 10px;
    }
    .badge {
      padding: 1px 6px; border-radius: 4px;
      background: var(--bg-3); color: var(--ink-3);
      border: 1px solid var(--line);
    }
    .badge.cat { color: var(--accent); border-color: oklch(0.78 0.14 70 / 0.3); }
    .badge.status-available { color: var(--ok); border-color: oklch(0.78 0.14 145 / 0.4); }
    .badge.status-beta { color: var(--warn); border-color: oklch(0.7 0.18 35 / 0.4); }
    .badge.status-research { color: var(--accent-3); border-color: oklch(0.72 0.18 330 / 0.4); }
    .badge.budget { color: var(--accent-2); border-color: oklch(0.78 0.12 195 / 0.3); }
    .card-foot {
      display: flex; align-items: center; gap: 8px;
      padding-top: 8px; margin-top: 4px;
      border-top: 1px solid var(--line);
      font-size: 11px; color: var(--ink-3);
    }
    .toggle {
      position: relative;
      width: 32px; height: 18px;
      background: var(--bg-3); border: 1px solid var(--line-2);
      border-radius: 999px; cursor: pointer;
      transition: background 0.15s;
      flex-shrink: 0;
    }
    .toggle::after {
      content: ''; position: absolute;
      top: 1px; left: 1px;
      width: 12px; height: 12px;
      background: var(--ink-3); border-radius: 50%;
      transition: transform 0.15s, background 0.15s;
    }
    .toggle.on { background: var(--accent); border-color: var(--accent); }
    .toggle.on::after { background: #1a0f00; transform: translateX(14px); }
    .events {
      font-family: var(--mono); font-size: 10px; color: var(--ink-3);
      flex: 1;
    }
    .empty {
      padding: 40px;
      text-align: center; color: var(--ink-3);
      font-size: 13px;
    }
  `;

  override connectedCallback(): void {
    super.connectedCallback();
    effect(() => { activations.value; query.value; activeCat.value; statusFilter.value; this.renderTick++; });
  }

  private isActive(id: string): boolean {
    return activations.value.find((a) => a.id === id)?.active === true;
  }

  private toggle(app: AppManifest): void {
    const next = activations.value.map((a) => a.id === app.id ? { ...a, active: !a.active, lastActivatedAt: Date.now() } : a);
    activations.value = next;
    pushLog(this.isActive(app.id) ? 'ok' : 'info', `app <span class="k">${app.id}</span> deactivated`);
  }

  private filtered(): AppManifest[] {
    let list = APPS;
    if (activeCat.value !== 'all') list = list.filter((a) => a.category === activeCat.value);
    if (statusFilter.value !== 'all') list = list.filter((a) => a.status === statusFilter.value);
    if (query.value.trim()) {
      list = list
        .map((a) => ({ a, s: fuzzyMatch(query.value, a) }))
        .filter((x) => x.s > 0)
        .sort((a, b) => b.s - a.s)
        .map((x) => x.a);
    }
    return list;
  }

  private categoryCounts(): Record<string, number> {
    const counts: Record<string, number> = { all: APPS.length };
    for (const k of Object.keys(CATEGORIES)) counts[k] = 0;
    for (const a of APPS) counts[a.category] = (counts[a.category] ?? 0) + 1;
    return counts;
  }

  override render() {
    const list = this.filtered();
    const counts = this.categoryCounts();
    const activeCount = activations.value.filter((a) => a.active).length;
    return html`
      <div class="head">
        <div class="ttl">
          App Store
          <small>${APPS.length} edge apps · ${activeCount} active</small>
        </div>
        <input class="search" id="app-search" placeholder="Search by name, tag, or category…"
          .value=${query.value}
          @input=${(e: Event) => { query.value = (e.target as HTMLInputElement).value; }} />
      </div>

      <div class="filters">
        <span class="chip ${activeCat.value === 'all' ? 'on' : ''}"
          @click=${() => activeCat.value = 'all'}>
          All<span class="count">${counts.all}</span>
        </span>
        ${(Object.keys(CATEGORIES) as AppCategory[]).map((k) => html`
          <span class="chip ${activeCat.value === k ? 'on' : ''}"
            @click=${() => activeCat.value = k}>
            <span class="swatch" style=${`background:${CATEGORIES[k].color}`}></span>
            ${CATEGORIES[k].label}
            <span class="count">${counts[k] ?? 0}</span>
          </span>
        `)}
        <span style="flex:1; min-width:8px"></span>
        <span class="chip ${statusFilter.value === 'all' ? 'on' : ''}" @click=${() => statusFilter.value = 'all'}>any</span>
        <span class="chip ${statusFilter.value === 'available' ? 'on' : ''}" @click=${() => statusFilter.value = 'available'}>available</span>
        <span class="chip ${statusFilter.value === 'beta' ? 'on' : ''}" @click=${() => statusFilter.value = 'beta'}>beta</span>
        <span class="chip ${statusFilter.value === 'research' ? 'on' : ''}" @click=${() => statusFilter.value = 'research'}>research</span>
      </div>

      ${list.length === 0
        ? html`<div class="empty">No apps match the current filters.</div>`
        : html`<div class="grid">${list.map((app) => this.card(app))}</div>`}
    `;
  }

  private card(app: AppManifest) {
    const active = this.isActive(app.id);
    const cat = CATEGORIES[app.category];
    return html`
      <div class="card ${active ? 'active' : ''}" data-app-id=${app.id}>
        <div class="card-h">
          <span class="swatch" style=${`background:${cat.color}`}></span>
          <span class="name">${app.name}</span>
        </div>
        <div class="summary">${app.summary}</div>
        <div class="meta">
          <span class="badge cat">${cat.label}</span>
          <span class="badge status-${app.status}">${app.status}</span>
          ${app.budget ? html`<span class="badge budget">budget ${app.budget}</span>` : ''}
          ${app.adr ? html`<span class="badge">${app.adr}</span>` : ''}
          ${app.events?.length ? html`<span class="badge">events ${app.events.join('·')}</span>` : ''}
        </div>
        <div class="card-foot">
          <span class="events">${app.crate}</span>
          <span class="toggle ${active ? 'on' : ''}" role="switch"
            aria-checked=${active}
            data-app-toggle=${app.id}
            @click=${() => this.toggle(app)}></span>
        </div>
      </div>
    `;
  }
}
