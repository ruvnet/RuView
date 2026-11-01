// SPDX-License-Identifier: MIT
// ruview-live — a Claude Code mod (function hooks) shipped inside @ruvnet/ruview.
//
// `/ruview` opens a pane beside the transcript with three views: an overview of
// the CSI nodes streaming to this machine and an optional ESPHome radar kit; a
// live CSI amplitude waterfall; and a radar fan with vitals charts (ADR-377,
// ADR-378). Every reading comes from the @ruvnet/ruview CLI this mod ships
// beside (`--json`), so the pane shows exactly what the tested tools return.
// Read-only: it never flashes, provisions or writes to a device.

import { commandsOf, cliPathOf, historyWith, MODES, modelOf, resultOf, settingsOf, spectrumWith, statusOf } from './model.mjs';
import { drain } from './anim.mjs';
import { ANIMATED, picturesOf, sizesOf, viewOf } from './views.mjs';

export * from './anim.mjs';
export * from './model.mjs';
export * from './raster.mjs';
export { ANIMATED, picturesOf, sizesOf, viewOf } from './views.mjs';

export const PANE_ID = 'ruview-live';
export const COMMAND = 'ruview';
const TICK_MS = 5000;
/** Body rows each view asks for when the pane sits inline above the prompt. */
export const VIEW_ROWS = Object.freeze({ overview: 16, waterfall: 30, radar: 26 });

/**
 * The open request: a dialog (focus + closeOnEscape + holdToasts) takes the
 * keyboard, so the view keys work at once; `rows` sizes the inline pane to
 * the view instead of a third of the screen (the dock ignores it).
 */
export const openArgsOf = (mode) => ({
  id: PANE_ID, title: 'RuView', focus: true, closeOnEscape: true, holdToasts: true, rows: VIEW_ROWS[mode] ?? VIEW_ROWS.overview,
});
/** Animation frame period: ~12 fps, well inside blit's 60 shown a second. */
export const FRAME_MS = 80;

/** Spectrum request for the waterfall: one bin per picture column (at most 128). */
export function spectrumOf(columns, rows) {
  const { width, height } = sizesOf(columns, rows);
  return { bins: Math.max(8, Math.min(128, width)), frames: Math.max(8, Math.min(256, height * 2)) };
}

/**
 * The mod entry: `/ruview [off|refresh|overview|waterfall|radar]`, the pane,
 * the timers, the status.
 * @param on the engine's registrar
 * @param options this plugin's userConfig values
 */
export function register(on, options = {}) {
  const settings = settingsOf(options);
  let host = null;
  let isOpen = false;
  let busy = false;
  let model = null;
  let history = null;
  let spectrum = [];
  let mode = 'overview';
  let nodeIndex = 0;
  let size = { columns: 100, rows: 30 };
  let stopTimer = null;
  let stopTick = null;
  let stopAnim = null;
  let animating = false;
  let lastFrame = null;
  let lag = {};
  let cliPath = null;

  const intervalOf = () => (mode === 'overview' ? settings.refreshMs : settings.liveRefreshMs);
  const stop = () => {
    if (stopTimer) { stopTimer(); stopTimer = null; }
    if (stopTick) { stopTick(); stopTick = null; }
    if (stopAnim) { stopAnim(); stopAnim = null; }
  };
  const cancelOf = (t) => (typeof t === 'function' ? t : () => t?.cancel?.());

  async function runCli(args) {
    if (!args) return null;
    try {
      return await host.run(['node', cliPath, ...args], { timeoutMs: (settings.captureSeconds + 30) * 1000 });
    } catch (error) {
      return { exitCode: -1, stdout: '', stderr: String(error?.message || error) };
    }
  }

  async function refresh() {
    if (!host || busy) return;
    busy = true;
    host.invalidate();
    try {
      const commands = commandsOf(settings, {
        live: mode !== 'overview',
        spectrum: mode === 'waterfall' ? spectrumOf(size.columns, size.rows) : null,
      });
      const [capture, radar] = await Promise.all([runCli(commands.capture), runCli(commands.radar)]);
      // $.clock.now() resolves a promise of epoch milliseconds.
      model = modelOf(resultOf(capture), resultOf(radar), await host.now());
      history = historyWith(history, model);
      spectrum = spectrumWith(spectrum, model.spectrum);
      lag = Object.fromEntries(Object.entries(lag).filter(([key]) => spectrum.some((s) => s.key === key)));
      // New frames join the replay queue: shown at their arrival rate, not at once.
      for (const s of model.spectrum) {
        const held = spectrum.find((x) => x.key === s.key)?.frames.length ?? 0;
        lag[s.key] = Math.min(held, (lag[s.key] ?? 0) + s.frames.length);
      }
      host.status(statusOf(model));
    } finally {
      busy = false;
      host.invalidate();
    }
  }

  /** Options shared by the full drawing and each animation frame. */
  const drawOpts = (t) => ({ mode, nodeIndex, lag, history, t, columns: size.columns, rows: size.rows });
  const shownModel = () => (model ? { ...model, spectrum } : null);

  /** One animation frame: repaint each mounted animated Raster in place. */
  async function animate() {
    if (!host || animating) return;
    animating = true;
    try {
      // Real time, the clock the full render reads too: pulses keep the
      // reported rates and the replay keeps up even when a frame fires late.
      const t = await host.now();
      if (!Number.isFinite(t)) return;
      const dt = Number.isFinite(lastFrame) ? Math.max(0, Math.min(1000, t - lastFrame)) : 0;
      lastFrame = t;
      for (const s of spectrum) lag[s.key] = drain(lag[s.key], s.rateHz, dt);
      const pics = picturesOf(shownModel(), drawOpts(t));
      // Fire and forget: a blit resolves only once a frame is painted, and the
      // surface folds blits between frames anyway, so nothing waits on one.
      for (const key of ANIMATED) {
        if (!pics[key]) continue;
        const { columns, rows, cells } = pics[key].grid.toRaster(key);
        Promise.resolve(host.blit({ requestId: PANE_ID, key, cells, columns, rows })).catch(() => undefined);
      }
    } finally {
      animating = false;
    }
  }

  /** Start the refresh timer, the age tick and the animation, and fetch now, unless already polling. */
  function startPolling() {
    isOpen = true;
    if (stopTimer) return;
    stopTimer = cancelOf(host.every(intervalOf(), () => { void refresh(); }));
    stopTick = cancelOf(host.every(TICK_MS, () => host.invalidate()));
    stopAnim = cancelOf(host.every(FRAME_MS, () => { void animate(); }));
    void refresh();
  }

  /** Switch view; the live views poll faster, so the timer restarts. */
  function setMode(next) {
    if (!MODES.includes(next) || next === mode) return;
    mode = next;
    if (isOpen && host) {
      // Each open sets the size anew: re-request rows for this view, keeping the keys.
      void host.open(openArgsOf(mode)).catch(() => undefined);
      stop();
      startPolling();
    } else host?.invalidate();
  }

  async function open(initialMode) {
    if (MODES.includes(initialMode)) mode = initialMode;
    await host.open(openArgsOf(mode));
    stop();
    startPolling();
  }

  async function close() {
    stop();
    isOpen = false;
    await host.close({ id: PANE_ID }).catch(() => undefined);
  }

  on('session.start', async ($, e, next) => {
    cliPath = cliPathOf($.plugin.root);
    host = {
      run: (argv, init) => $.process.run(argv, init),
      every: (ms, fn) => $.clock.every(ms, fn),
      now: () => $.clock.now(),
      open: (pane) => $.ui.open(pane),
      close: (pane) => $.ui.close(pane),
      status: (text) => $.ui.status(text),
      invalidate: () => $.ui.invalidate('ui.render'),
      blit: (args) => $.ui.blit(args),
    };
    await $.command.register({ name: COMMAND, description: 'RuView live sensing pane: CSI nodes, CSI waterfall and radar', argumentHint: '[waterfall|radar|refresh|off]' }).catch(() => undefined);
    return next(e);
  });

  on('command.run', { command: COMMAND }, async ($, e, next) => {
    if (!host) return next(e);
    const arg = String(e.args || '').trim().toLowerCase();
    if (arg === 'off') { await close(); host.status(undefined); return { text: 'RuView pane closed.' }; }
    if (arg === 'refresh') { await refresh(); return { text: statusOf(model) }; }
    if (MODES.includes(arg)) {
      if (isOpen) {
        if (arg === mode) await host.open(openArgsOf(mode)).catch(() => undefined);
        else setMode(arg);
      } else await open(arg);
      return { text: `RuView pane: ${arg} view.` };
    }
    if (isOpen) { await close(); return { text: 'RuView pane closed.' }; }
    await open();
    return { text: `RuView pane open: nodes on UDP ${settings.udpPort}${settings.radarHost ? `, radar ${settings.radarHost}` : ''}; keys 1 overview · 2 CSI waterfall · 3 radar.` };
  });

  on('ui.render', { component: 'Pane' }, async ($, e, next) => {
    if (e.requestId !== PANE_ID) return next(e);
    // A reload (hot reload, or a resumed session) re-runs register with fresh
    // variables while the engine keeps the pane open: drawing it means it is
    // open, so resume polling instead of waiting for the first capture forever.
    if (host && !stopTimer) startPolling();
    size = {
      columns: e.props?.bodyColumns ?? e.viewport?.columns ?? 100,
      rows: e.props?.scroll?.bodyRows ?? e.viewport?.rows ?? 30,
    };
    const ui = await $.ui.resolve(e);
    const now = await $.clock.now();
    return viewOf(ui, shownModel(), {
      ...drawOpts(now), refreshMs: settings.refreshMs, liveRefreshMs: settings.liveRefreshMs, busy, now,
      focused: e.props?.isFocused !== false,
      udpPort: settings.udpPort, radarConfigured: Boolean(settings.radarHost),
      onRefresh: () => { void refresh(); },
      onClose: () => { void close(); },
      onMode: (m) => setMode(m),
      onNextNode: () => { nodeIndex += 1; host?.invalidate(); },
    });
  });

  on('ui.close', { id: PANE_ID }, async ($, e, next) => {
    stop();
    isOpen = false;
    return next(e);
  });

  on('session.end', async ($, e, next) => {
    stop();
    return next(e);
  });
}
