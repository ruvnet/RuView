// Debug Tab — raw data collection, calibration control, and live pipeline
// trace. Shows every precondition the certified-vitals path checks, the raw
// estimator output, buffer fill, and lets you start/stop calibration from
// the browser.
//
// Endpoints used:
//   GET  /api/v1/vital-signs      — raw + certified + gates + certification
//   GET  /api/v1/calibration/status
//   POST /api/v1/calibration/start?source_node_id=1
//   POST /api/v1/calibration/stop
//   POST /api/v1/calibration/cancel
//   GET  /api/v1/sensing/latest   — node / feature snapshot

const POLL_MS = 2000;
// Fetch cadence for the server-side fall capture record (the record itself is
// per-frame; this only controls how often the waveform repaints).
const FALL_CAPTURE_POLL_MS = 1000;

export class DebugTab {
  constructor(container) {
    this.container = container;
    this._timer = null;
    this._history = [];       // rolling raw-vitals samples
    this._maxHistory = 30;
    this._certHistory = [];   // rolling certified-vitals samples (published only)
    this._maxCertHistory = 500;
    this._calState = null;    // last calibration/status response
    this._vitalsState = null; // last vital-signs response
    this._sensingState = null;
    this._pendingStop = null; // identity needed for stop
    this._rf = [];            // rolling RF-feature samples for the chart
    this._maxRf = 120;        // ~2 min at 1 s/sample
    this._chartTimer = null;  // faster poll that drives the CSI/RF chart
    this._lastSpectrum = null;// latest per-subcarrier amplitude vector (if any)
    this._rawPktTimer = null; // 1 s poll of the decoded UDP packet log
    this._rawPktPaused = false;
    this._rawPktState = null; // last /api/v1/debug/raw-packets response
    // Magic types shown in the raw-packet table. Defaults to the four packet
    // types the ESP32 edge emits for sensing/vitals debugging.
    this._rawPktSelected = new Set([
      '0xC5110001', '0xC5110002', '0xC5110003', '0xC5110005',
    ]);
    this._rawPktScrollInit = false;
    // Vitals trend curves. `_br1`/`_hr1` come from the server CSI estimator
    // (magic 0xC5110001 via /api/v1/vital-signs); `_br2`/`_hr2` come from the
    // ESP32 edge vitals packets (magic 0xC5110002, decoded in the raw-packet
    // log). Each point is {t: unix_ms, v: bpm}.
    this._br1 = [];
    this._hr1 = [];
    this._br2 = [];
    this._hr2 = [];
    this._edgeState = null;  // latest decoded 0xC5110002 packet
    this._lastEdgeTs = null; // dedupe edge packets by their own timestamp
    this._fallState = null;  // last /api/v1/debug/fall response (server edge-equivalent)
    // Fall capture: user-armed recording of the fall-detector scores, shown
    // as the stillness waveform below the live strip.
    this._fallCap = {
      on: false, timer: null, data: [], node: null, full: false,
      // Fallback until the server reports the thresholds in force.
      thresholds: { spike: 3.5, still: 0.09 },
    };
    this._rfWindowMs = 120000; // chart time window: last 2 min
    // Waterfall: one column per 1 s poll of the 56-bin amplitude vector.
    this._wf = [];
    this._maxWf = 150;       // ~2.5 min at 1 s/column
    this._baseline = this._loadBaseline(); // {t, amps} captured by the user
  }

  async init() {
    this._buildDOM();
    this._startPolling();
    this._startChart();
    this._startRawPackets();
  }

  dispose() {
    this._stopPolling();
    this._stopChart();
    this._stopRawPackets();
    this._stopFallCapture();
  }

  // ── Polling ──────────────────────────────────────────────────────────────

  _startPolling() {
    this._stopPolling();
    this._refresh();
    this._timer = setInterval(() => this._refresh(), POLL_MS);
  }

  _stopPolling() {
    if (this._timer) {
      clearInterval(this._timer);
      this._timer = null;
    }
  }

  async _refresh() {
    const results = await Promise.allSettled([
      fetch('/api/v1/vital-signs', { cache: 'no-store' }).then(r => r.json()),
      fetch('/api/v1/calibration/status', { cache: 'no-store' }).then(r => r.json()),
      fetch('/api/v1/sensing/latest', { cache: 'no-store' }).then(r => r.json()),
      fetch('/api/v1/debug/fall', { cache: 'no-store' }).then(r => r.json()),
    ]);
    if (results[0].status === 'fulfilled') {
      this._vitalsState = results[0].value;
      this._recordHistory(results[0].value);
      this._recordCertHistory(results[0].value);
    }
    if (results[1].status === 'fulfilled') this._calState = results[1].value;
    if (results[2].status === 'fulfilled') this._sensingState = results[2].value;
    if (results[3].status === 'fulfilled') this._fallState = results[3].value;

    this._renderVitals();
    this._renderCertTrace();
    this._renderFall();
    this._renderCalStatus();
    this._renderBuffer();
    this._renderHistory();
    this._renderCertHistory();
    this._renderSensing();
  }

  // ── History (rolling table) ──────────────────────────────────────────────

  _recordHistory(vs) {
    const raw = vs?.raw_vital_signs || {};
    const cert = vs?.certification || {};
    const now = new Date();
    this._history.unshift({
      time: now.toTimeString().slice(0, 8),
      br: raw.breathing_rate_bpm,
      hr: raw.heart_rate_bpm,
      brc: raw.breathing_confidence,
      hrc: raw.heartbeat_confidence,
      q: raw.signal_quality,
      pc: cert.room_person_count,
      reason: cert.reason,
    });
    if (this._history.length > this._maxHistory) {
      this._history.length = this._maxHistory;
    }
  }

  _renderHistory() {
    const tbody = this.container.querySelector('#dbgHistoryBody');
    if (!tbody) return;
    tbody.innerHTML = this._history.map(r => {
      const fmt = (v, nd = 1) => v == null ? '--' : Number(v).toFixed(nd);
      const cls = (ok) => ok ? 'dbg-ok' : 'dbg-bad';
      const qOk = r.q != null && r.q >= 0.40;
      const brcOk = r.brc != null && r.brc >= 0.55;
      const hrcOk = r.hrc != null && r.hrc >= 0.55;
      return `<tr>
        <td>${r.time}</td>
        <td class="${cls(r.br != null)}">${fmt(r.br)}</td>
        <td class="${cls(r.hr != null)}">${fmt(r.hr)}</td>
        <td class="${cls(brcOk)}">${fmt(r.brc, 2)}</td>
        <td class="${cls(hrcOk)}">${fmt(r.hrc, 2)}</td>
        <td class="${cls(qOk)}">${fmt(r.q, 2)}</td>
        <td>${r.pc ?? '--'}</td>
        <td class="dbg-reason">${r.reason || '--'}</td>
      </tr>`;
    }).join('');
  }

  // ── Certified vitals history (published rows only) ───────────────────────
  // A row is recorded only when the server actually published vitals
  // (`authority === 'explicit_calibration'`): every gate passed and the
  // values are the certified ones a consumer may trust. Rows missing a gate
  // never appear here — they stay in Raw Vitals History with their reason.

  _recordCertHistory(vs) {
    if (!vs || vs.authority !== 'explicit_calibration') return;
    const pub = vs.vital_signs || {};
    if (pub.breathing_rate_bpm == null && pub.heart_rate_bpm == null) return;
    const cert = vs.certification || {};
    const now = new Date();
    this._certHistory.unshift({
      time: now.toTimeString().slice(0, 8),
      date: now.toLocaleDateString('sv-SE'), // ISO local date for the CSV export
      br: pub.breathing_rate_bpm,
      hr: pub.heart_rate_bpm,
      brc: pub.breathing_confidence,
      hrc: pub.heartbeat_confidence,
      q: pub.signal_quality,
      pc: cert.room_person_count,
      authority: vs.authority,
      tick: vs.tick,
      source: vs.source,
    });
    if (this._certHistory.length > this._maxCertHistory) {
      this._certHistory.length = this._maxCertHistory;
    }
  }

  _renderCertHistory() {
    const tbody = this.container.querySelector('#dbgCertHistoryBody');
    if (!tbody) return;
    if (!this._certHistory.length) {
      tbody.innerHTML = '<tr><td colspan="9" style="color:#555;padding:8px;">尚无通过认证的样本 — 所有门控通过（含人在场 count=1）后才会记录。</td></tr>';
      return;
    }
    const fmt = (v, nd = 1) => v == null ? '--' : Number(v).toFixed(nd);
    // A "--" rate in a published row means that vital's own confidence gate
    // (≥ 0.55) was closed while the other vital passed. Surface the failed
    // confidence on hover so the blank is self-explanatory.
    // 只显示最近 20 行；完整 500 行仍在 _certHistory 里供导出。
    const view = this._certHistory.slice(0, 20);
    const cell = (v, conf, label) => v == null
      ? `<span title="未发布：${label}Conf ${fmt(conf, 2)} < 0.55" style="color:#667;cursor:help;">--</span>`
      : fmt(v);
    tbody.innerHTML = view.map(r => `<tr>
        <td>${r.time}</td>
        <td class="dbg-ok">${cell(r.br, r.brc, 'Br')}</td>
        <td class="dbg-ok">${cell(r.hr, r.hrc, 'Heart')}</td>
        <td>${fmt(r.brc, 2)}</td>
        <td>${fmt(r.hrc, 2)}</td>
        <td>${fmt(r.q, 2)}</td>
        <td>${r.pc ?? '--'}</td>
        <td style="font-size:10px;color:#667;">${r.authority || '--'}</td>
        <td style="font-size:10px;color:#667;">${r.tick ?? '--'}</td>
      </tr>`).join('');
  }

  _exportCertHistory(kind) {
    const rows = this._certHistory.slice().reverse(); // oldest → newest
    let blob, name;
    if (kind === 'csv') {
      const head = ['date', 'time', 'breathing_bpm', 'heart_bpm', 'breath_conf', 'heart_conf', 'signal_quality', 'person_count', 'authority', 'tick', 'source'];
      const esc = (v) => {
        if (v == null) return '';
        const s = String(v);
        return /[",\n]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s;
      };
      const lines = [head.join(',')].concat(rows.map(r =>
        [r.date, r.time, r.br, r.hr, r.brc, r.hrc, r.q, r.pc, r.authority, r.tick, r.source].map(esc).join(',')));
      blob = new Blob(['\uFEFF' + lines.join('\n') + '\n'], { type: 'text/csv;charset=utf-8' });
      name = 'ruview-certified-vitals.csv';
    } else {
      blob = new Blob([JSON.stringify(rows, null, 2) + '\n'], { type: 'application/json' });
      name = 'ruview-certified-vitals.json';
    }
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  // ── Vitals (snapshot) ────────────────────────────────────────────────────

  _renderVitals() {
    const vs = this._vitalsState;
    if (!vs) return;
    const raw = vs.raw_vital_signs || {};
    const certBlock = vs.vital_signs || {};
    const g = vs.gates || {};
    const set = (id, txt) => {
      const el = this.container.querySelector(id);
      if (el) el.textContent = txt;
    };
    const f = (x, nd = 1) => x == null ? '--' : Number(x).toFixed(nd);

    set('#dbgRawBR', f(raw.breathing_rate_bpm) + ' bpm');
    set('#dbgRawHR', f(raw.heart_rate_bpm) + ' bpm');
    set('#dbgRawBRC', f(raw.breathing_confidence, 2));
    set('#dbgRawHRC', f(raw.heartbeat_confidence, 2));
    set('#dbgRawQ', f(raw.signal_quality, 2));

    set('#dbgCertBR', f(certBlock.breathing_rate_bpm) + ' bpm');
    set('#dbgCertHR', f(certBlock.heart_rate_bpm) + ' bpm');
    set('#dbgAuth', vs.authority || '--');
    const cert = vs.certification || {};
    const reason = vs.abstention_reason || '—';
    set('#dbgReason', reason === 'room_person_count_not_one' && cert.room_person_count != null
      ? `${reason} (count=${cert.room_person_count})`
      : reason);

    set('#dbgGPersonCount', `${g.person_count ?? '--'} ${g.person_count_ok ? '✓' : '✗'}`);
    set('#dbgGQual', `${f(g.signal_quality, 2)} ${g.signal_quality_ok ? '✓' : '✗'}`);
    // The gates object carries only the pass/fail flags; the numeric
    // confidences live in raw_vital_signs.
    set('#dbgGBrc', `${f(raw.breathing_confidence, 2)} ${g.breathing_confidence_ok ? '✓' : '✗'}`);
    set('#dbgGHrc', `${f(raw.heartbeat_confidence, 2)} ${g.heartbeat_confidence_ok ? '✓' : '✗'}`);

    const badge = this.container.querySelector('#dbgAuthorityBadge');
    if (badge) {
      const ok = vs.authority === 'explicit_calibration';
      badge.textContent = ok ? 'CERTIFIED' : 'ABSTAINED';
      badge.className = 'dbg-badge ' + (ok ? 'dbg-badge-ok' : 'dbg-badge-warn');
    }
  }

  // ── Fall detection (server-side edge equivalent) ─────────────────────────
  // The server runs the firmware's #263 fall algorithm on the same raw
  // 0xC5110001 CSI the board's edge runs on. This card shows the server's
  // state machine so the two implementations can be compared live: the
  // firmware's own verdict rides to the host as flags bit1 in 0xC5110002.

  _renderFall() {
    const f = this._fallState;
    const container = this.container.querySelector('#dbgFallCard');
    if (!container) return;
    if (!f) {
      container.innerHTML = '<div style="font-size:11px;color:#667;">fall debug unavailable</div>';
      return;
    }
    const entries = Object.entries(f.nodes || {});
    if (!entries.length) {
      container.innerHTML = '<div style="font-size:11px;color:#667;">No ESP32 nodes yet — waiting for 0xC5110001 frames.</div>';
      return;
    }
    const set = (id, txt) => {
      const el = this.container.querySelector(id);
      if (el) el.textContent = txt;
    };
    // Primary node = the one the certified pipeline is bound to (node 1), else first.
    const [nodeId, node] = entries.find(([id]) => id === '1') || entries[0];
    const st = node.status || {};

    const badge = this.container.querySelector('#dbgFallBadge');
    if (badge) {
      badge.textContent = st.detected ? 'FALL' : 'CLEAR';
      badge.className = 'dbg-badge ' + (st.detected ? 'dbg-badge-err' : 'dbg-badge-ok');
    }
    set('#dbgFallNode', `node ${nodeId}`);
    set('#dbgFallAccel', st.spike_score != null ? Number(st.spike_score).toFixed(2) : '--');
    set('#dbgFallThresh', st.spike_threshold != null ? Number(st.spike_threshold).toFixed(1) : '--');
    set('#dbgFallConsec', `${st.spike_consec ?? '--'} / 5 连续帧 ≥ ${st.spike_threshold ?? '--'}`);
    set('#dbgFallStill', st.still_avg != null
      ? `avg ${(st.still_avg * 100).toFixed(2)}% / 确认线 ${(st.still_threshold * 100).toFixed(1)}% 持续 12s`
      : '--');
    set('#dbgFallPending', st.pending ? 'YES（等待静止确认）' : 'no');
    const lastAlert = this.container.querySelector('#dbgFallLastAlert');
    if (lastAlert) {
      lastAlert.textContent = st.last_alert_unix_ms
        ? new Date(st.last_alert_unix_ms).toLocaleTimeString()
        : '--';
    }

    // Spike-score trace with threshold line (DPR-aware, CSS-pixel drawing).
    const canvas = this.container.querySelector('#dbgFallChart');
    if (!canvas || !canvas.getContext) return;
    const hist = node.score_history || [];
    const cssW = canvas.clientWidth || 300;
    const cssH = 120;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    if (canvas.width !== Math.round(cssW * dpr) || canvas.height !== Math.round(cssH * dpr)) {
      canvas.width = Math.round(cssW * dpr);
      canvas.height = Math.round(cssH * dpr);
      canvas.style.height = cssH + 'px';
    }
    const ctx = canvas.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cssW, cssH);
    ctx.fillStyle = '#12121e';
    ctx.fillRect(0, 0, cssW, cssH);

    const padL = 36, padR = 6, padT = 8, padB = 16;
    const plotW = cssW - padL - padR, plotH = cssH - padT - padB;
    const thresh = st.spike_threshold || 8;
    const maxAccel = Math.max(thresh * 1.3, ...hist, 1);
    const font = '10px ui-monospace, monospace';

    // Grid + y labels
    ctx.strokeStyle = '#26263a';
    ctx.fillStyle = '#667';
    ctx.lineWidth = 1;
    ctx.font = font;
    for (let gy = 0; gy <= 4; gy++) {
      const v = (maxAccel * gy) / 4;
      const y = padT + plotH - (v / maxAccel) * plotH;
      ctx.beginPath(); ctx.moveTo(padL, y); ctx.lineTo(cssW - padR, y); ctx.stroke();
      ctx.textAlign = 'right';
      ctx.fillText(v.toFixed(0), padL - 4, y + 3);
    }

    // Threshold line
    const ty = padT + plotH - (thresh / maxAccel) * plotH;
    ctx.strokeStyle = '#c0392b';
    ctx.setLineDash([4, 3]);
    ctx.beginPath(); ctx.moveTo(padL, ty); ctx.lineTo(cssW - padR, ty); ctx.stroke();
    ctx.setLineDash([]);
    ctx.fillStyle = '#c0392b';
    ctx.textAlign = 'left';
    ctx.fillText(`spike ${thresh.toFixed(0)}`, padL + 2, ty - 3);

    // Spike-score trace (oldest → newest)
    if (hist.length >= 2) {
      ctx.strokeStyle = '#4db8ff';
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      hist.forEach((v, i) => {
        const x = padL + (i / (hist.length - 1)) * plotW;
        const y = padT + plotH - Math.min(v, maxAccel) / maxAccel * plotH;
        i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
      });
      ctx.stroke();
    } else {
      ctx.fillStyle = '#667';
      ctx.textAlign = 'center';
      ctx.fillText('waiting for accel samples…', cssW / 2, padT + plotH / 2);
    }
  }

  // ── Fall capture (user-armed recording + stillness waveform) ────────────
  // Recording runs on the server, one record per processed CSI frame
  // (POST /api/v1/debug/fall/capture); the tab only fetches the record.
  // Browser-side polling of the live status sampled at 5 Hz and missed the
  // 5-frame spike runs the detector actually triggers on.

  async _fallCaptureAction(action) {
    const res = await fetch('/api/v1/debug/fall/capture', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action }),
    });
    const body = await res.json();
    if (!body.success) throw new Error(body.error || `${action} failed`);
    return body;
  }

  async _toggleFallCapture() {
    const cap = this._fallCap;
    const info = this.container.querySelector('#dbgFallCapInfo');
    try {
      if (cap.on) {
        this._stopFallCapture();
        await this._fallCaptureAction('stop');
        await this._pollFallCapture(); // pull the final frames
      } else {
        const r = await this._fallCaptureAction('start');
        if (!r.nodes) throw new Error('没有在线节点');
        cap.data = [];
        cap.full = false;
        cap.on = true;
        cap.timer = setInterval(() => this._pollFallCapture(), FALL_CAPTURE_POLL_MS);
      }
    } catch (err) {
      this._stopFallCapture();
      if (info) info.textContent = '捕捉控制失败: ' + err.message;
      return;
    }
    this._renderFallCaptureControls();
  }

  _stopFallCapture() {
    const cap = this._fallCap;
    if (cap.timer) {
      clearInterval(cap.timer);
      cap.timer = null;
    }
    cap.on = false;
  }

  async _clearFallCapture() {
    this._stopFallCapture();
    try { await this._fallCaptureAction('clear'); } catch (err) { /* local clear still applies */ }
    this._fallCap.data = [];
    this._fallCap.full = false;
    this._renderFallCaptureControls();
    this._renderFallCapture();
  }

  _exportFallCapture() {
    const cap = this._fallCap;
    if (!cap.data.length) return;
    const span = cap.data[cap.data.length - 1].t - cap.data[0].t;
    const payload = {
      schema: 'ruview.fall-capture.v2',
      exported_at: new Date().toISOString(),
      source: 'server full-rate capture (one record per CSI frame)',
      node_id: cap.node,
      frames: cap.data.length,
      effective_rate_hz: span > 0 ? (cap.data.length - 1) / span : null,
      thresholds: cap.thresholds,
      data: cap.data,
    };
    const blob = new Blob([JSON.stringify(payload, null, 2) + '\n'], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `ruview-fall-capture-${new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  async _importFallCapture(file) {
    const info = this.container.querySelector('#dbgFallCapInfo');
    try {
      const obj = JSON.parse(await file.text());
      const rows = Array.isArray(obj) ? obj : obj.data;
      if (!Array.isArray(rows) || !rows.length) throw new Error('文件里没有数据行');
      const data = rows.map(r => ({
        t: Number(r.t) || 0,
        spike: Number(r.spike) || 0,
        still: Number(r.still) || 0,
        still_inst: Number(r.still_inst) || 0,
        pending: !!r.pending,
        detected: !!r.detected,
      })).sort((a, b) => a.t - b.t);
      this._stopFallCapture();
      this._fallCap.data = data;
      this._fallCap.node = obj.node_id ?? null;
      // v1 files carried hard-coded thresholds; trust only what the file says.
      const th = obj.thresholds || {};
      this._fallCap.thresholds = {
        spike: Number(th.spike) || this._fallCap.thresholds.spike,
        still: Number(th.still) || this._fallCap.thresholds.still,
      };
      this._renderFallCaptureControls();
      this._renderFallCapture();
      if (info) info.textContent = `已导入 ${data.length} 帧（${data[data.length - 1].t.toFixed(1)} s）`;
    } catch (err) {
      if (info) info.textContent = '导入失败: ' + err.message;
    }
  }

  async _pollFallCapture() {
    try {
      const d = await (await fetch('/api/v1/debug/fall?data=1', { cache: 'no-store' })).json();
      const nodes = d.nodes || {};
      const cap = this._fallCap;
      // Stay on one node for the whole record; prefer node 1, else the first.
      if (cap.node == null || !nodes[cap.node]) {
        cap.node = nodes['1'] ? '1' : (Object.keys(nodes)[0] ?? null);
      }
      const n = cap.node != null ? nodes[cap.node] : null;
      if (!n) return;
      const st = n.status || {};
      cap.thresholds = {
        spike: Number(st.spike_threshold) || cap.thresholds.spike,
        still: Number(st.still_threshold) || cap.thresholds.still,
      };
      const rows = (n.capture && n.capture.data) || [];
      const t0 = rows.length ? rows[0].t_unix_ms : 0;
      cap.data = rows.map(r => ({
        t: (r.t_unix_ms - t0) / 1000,
        t_unix_ms: r.t_unix_ms,
        spike: r.spike || 0,
        still: r.still_avg || 0,      // the confirmation criterion's input
        still_inst: r.still || 0,
        pending: !!r.pending,
        detected: !!r.detected,
      }));
      cap.full = !!(n.capture && n.capture.full);
      // The server auto-stops when its buffer is full.
      if (cap.on && n.capture && !n.capture.capturing) this._stopFallCapture();
      this._renderFallCapture();
      this._renderFallCaptureControls();
    } catch (err) { /* a missed poll is fine; the next one repaints */ }
  }

  _renderFallCaptureControls() {
    const cap = this._fallCap;
    const btn = this.container.querySelector('#dbgFallCapBtn');
    if (btn) btn.textContent = cap.on ? '停止捕捉' : '开始捕捉';
    const info = this.container.querySelector('#dbgFallCapInfo');
    if (info) {
      const node = cap.node != null ? `node ${cap.node} · ` : '';
      if (!cap.data.length) {
        info.textContent = cap.on
          ? `${node}捕捉中，等待首帧…`
          : '未捕捉 — 点「开始捕捉」录制波形（服务端逐帧）';
      } else {
        const dur = cap.data[cap.data.length - 1].t;
        const rate = dur > 0 ? ((cap.data.length - 1) / dur).toFixed(1) : '--';
        const state = cap.on ? '捕捉中' : (cap.full ? '缓冲已满，已自动停止' : '已停止');
        info.textContent = `${node}${state} — ${cap.data.length} 帧 / ${dur.toFixed(1)} s（${rate} Hz）`;
      }
    }
  }

  _renderFallCapture() {
    const canvas = this.container.querySelector('#dbgFallCapChart');
    if (!canvas || !canvas.getContext) return;
    const cap = this._fallCap;
    const ctx = canvas.getContext('2d');

    const cssW = canvas.clientWidth || 320;
    const spikePanelH = 120; // 与上方实时图同高
    const stillPanelH = 110;
    const cssH = spikePanelH + stillPanelH + 10 + 18;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    if (canvas.width !== Math.round(cssW * dpr) || canvas.height !== Math.round(cssH * dpr)) {
      canvas.width = Math.round(cssW * dpr);
      canvas.height = Math.round(cssH * dpr);
      canvas.style.height = cssH + 'px';
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = '#12121e';
    ctx.fillRect(0, 0, cssW, cssH);

    const data = cap.data;
    const padL = 44, padR = 8, gap = 10, padB = 18;
    const zPanelH = spikePanelH;
    const panelH = stillPanelH;
    const tMax = Math.max(30, data.length ? data[data.length - 1].t : 30);
    const font = '10px ui-monospace, monospace';

    const drawPanel = (y0, h, key, thresh, color, label, yFmt) => {
      // panel background + border
      ctx.strokeStyle = '#26263a';
      ctx.strokeRect(padL, y0, cssW - padL - padR, h);
      // y max/min labels
      ctx.fillStyle = '#667';
      ctx.font = font;
      ctx.textAlign = 'right';
      ctx.fillText(label, padL - 4, y0 + 10);
      // threshold line
      const ty = y0 + h - (thresh / yFmt.ymax) * h;
      ctx.strokeStyle = '#c0392b';
      ctx.setLineDash([4, 3]);
      ctx.beginPath(); ctx.moveTo(padL, ty); ctx.lineTo(cssW - padR, ty); ctx.stroke();
      ctx.setLineDash([]);
      ctx.fillStyle = '#c0392b';
      ctx.textAlign = 'left';
      ctx.fillText(yFmt.threshLabel, padL + 2, y0 + 10);
      // trace
      if (data.length >= 2) {
        // pending/detected shading
        for (const [flag, color2] of [['detected', 'rgba(231,76,60,0.18)'], ['pending', 'rgba(243,156,18,0.15)']]) {
          let runStart = null;
          for (let i = 0; i <= data.length; i++) {
            const on = i < data.length && data[i][flag];
            if (on && runStart === null) runStart = i;
            if ((!on || i === data.length) && runStart !== null) {
              const x0 = padL + (data[runStart].t / tMax) * (cssW - padL - padR);
              const x1 = padL + (data[i - 1].t / tMax) * (cssW - padL - padR);
              ctx.fillStyle = color2;
              ctx.fillRect(x0, y0, Math.max(2, x1 - x0), h);
              runStart = null;
            }
          }
        }
        ctx.strokeStyle = color;
        ctx.lineWidth = 1.3;
        ctx.beginPath();
        data.forEach((d, i) => {
          const x = padL + (d.t / tMax) * (cssW - padL - padR);
          const v = Math.min(d[key], yFmt.ymax);
          const y = y0 + h - (v / yFmt.ymax) * h;
          i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
        });
        ctx.stroke();
        ctx.lineWidth = 1;
      } else {
        ctx.fillStyle = '#667';
        ctx.textAlign = 'center';
        ctx.fillText('捕捉数据不足', padL + (cssW - padL - padR) / 2, y0 + h / 2);
      }
    };

    const xLabels = () => {
      ctx.fillStyle = '#667';
      ctx.font = font;
      ctx.textAlign = 'center';
      const step = tMax <= 30 ? 5 : tMax <= 60 ? 10 : tMax <= 120 ? 30 : 60;
      for (let t = 0; t <= tMax; t += step) {
        const x = padL + (t / tMax) * (cssW - padL - padR);
        ctx.fillText(`${t}s`, x, cssH - 5);
      }
    };

    // 上：尖峰 z；下：静止 Δs%（4 s 平均）。阈值取服务端实际生效值。
    const { spike: spikeTh, still: stillTh } = cap.thresholds;
    let spikeYmax = Math.max(spikeTh * 1.3, 1);
    for (const d of data) spikeYmax = Math.max(spikeYmax, d.spike || 0); // no spread: up to 3600 rows
    drawPanel(0, zPanelH, 'spike', spikeTh, '#4db8ff', 'z',
      { ymax: spikeYmax, threshLabel: `spike ${spikeTh}` });
    drawPanel(zPanelH + gap, panelH, 'still', stillTh, '#2ecc71', '|Δs|%',
      { ymax: Math.max(0.25, stillTh * 2), threshLabel: `still ${(stillTh * 100).toFixed(1)}%` });
    xLabels();
  }

  // ── Certification trace ──────────────────────────────────────────────────

  _renderCertTrace() {
    const vs = this._vitalsState;
    const c = vs?.certification;
    const container = this.container.querySelector('#dbgCertTrace');
    if (!container) return;

    if (!c) {
      container.innerHTML = '<div class="dbg-cert-reason">certification block unavailable (server too old)</div>';
      return;
    }

    const step = (label, value, ok) => {
      const icon = ok === true ? '✓' : ok === false ? '✗' : '·';
      const cls = ok === true ? 'dbg-ok' : ok === false ? 'dbg-bad' : 'dbg-unknown';
      return `<div class="dbg-cert-step ${cls}">
        <span class="dbg-cert-icon">${icon}</span>
        <span class="dbg-cert-label">${label}</span>
        <span class="dbg-cert-val">${value}</span>
      </div>`;
    };

    const hw = c.holdout_frames != null && c.runtime_window_size != null
      ? `${c.holdout_frames}/${c.runtime_window_size}`
      : '--';

    const html = [
      step('explicit_calibration_fresh', c.explicit_calibration_fresh, c.explicit_calibration_fresh),
      step('room_person_count == 1', c.room_person_count, c.room_person_count === 1),
      step('sequence_faults == 0', c.sequence_fault_node_count, c.sequence_fault_node_count === 0),
      step('grid_binding_present', c.grid_binding_present, c.grid_binding_present),
      step('runtime_window_size', c.runtime_window_size ?? 'null', c.runtime_window_size != null),
      step('holdout_ready', `${c.holdout_ready} (${hw})`, c.holdout_ready),
      step('presence_evidence_present', c.presence_evidence_present, c.presence_evidence_present),
      step('calibrated_person_count == 1', c.calibrated_person_count ?? 'null', c.calibrated_person_count === 1),
      step('signal_quality >= 0.40', `${Number(c.signal_quality ?? 0).toFixed(2)} ${c.signal_quality_ok ? '✓' : '✗'}`, c.signal_quality_ok),
      step('confidence >= 0.55', c.confidence_ok, c.confidence_ok),
    ].join('');

    container.innerHTML = html +
      `<div class="dbg-cert-reason">reason: <strong>${c.reason}</strong></div>`;
  }

  // ── Buffer fill ──────────────────────────────────────────────────────────

  _renderBuffer() {
    const vs = this._vitalsState;
    const b = vs?.buffer_status;
    if (!b) return;
    const bar = (len, cap) => {
      const pct = cap > 0 ? Math.min(100, (len / cap) * 100) : 0;
      return `<div class="dbg-bar-track"><div class="dbg-bar-fill" style="width:${pct.toFixed(1)}%"></div></div>`;
    };
    const el1 = this.container.querySelector('#dbgBufBreath');
    if (el1) {
      el1.innerHTML = `${b.breathing_samples}/${b.breathing_capacity} ${bar(b.breathing_samples, b.breathing_capacity)}`;
    }
    const el2 = this.container.querySelector('#dbgBufHeart');
    if (el2) {
      el2.innerHTML = `${b.heartbeat_samples}/${b.heartbeat_capacity} ${bar(b.heartbeat_samples, b.heartbeat_capacity)}`;
    }
  }

  // ── Calibration status ───────────────────────────────────────────────────

  _renderCalStatus() {
    const c = this._calState;
    if (!c) return;
    const set = (id, txt) => {
      const el = this.container.querySelector(id);
      if (el) el.textContent = txt;
    };
    const s = c.status || 'unknown';
    const collecting = s === 'collecting' || s === 'uncalibrated';
    const grid = c.grid_binding || {};
    const stale = collecting && (
      grid.status === 'stale'
      || (grid.latest_seen_ms != null && grid.latest_seen_ms > 5000)
    );

    const badge = this.container.querySelector('#dbgCalBadge');
    if (badge) {
      const cls = collecting ? 'dbg-badge-warn'
        : s === 'fresh' ? 'dbg-badge-ok'
        : s === 'stale' || s === 'expired' ? 'dbg-badge-err'
        : 'dbg-badge-info';
      badge.textContent = s.toUpperCase();
      badge.className = 'dbg-badge ' + cls;
    }

    const minFrames = Number(c.min_frames) || 0;
    const minDur = Number(c.min_duration_s) || 0;
    const frames = Number(c.frame_count) || 0;
    const elapsed = Number(c.elapsed_s) || 0;
    const fps = Number(c.frames_per_second) || 0;
    const framePct = minFrames > 0 ? Math.min(100, (frames / minFrames) * 100) : 0;
    const durPct = minDur > 0 ? Math.min(100, (elapsed / minDur) * 100) : 0;
    // Both gates must pass, so overall progress is the slower one.
    const pct = Math.min(framePct, durPct);
    const remaining = Math.max(0, minDur - elapsed);
    const ready = minFrames > 0 && frames >= minFrames && elapsed >= minDur;

    set('#dbgCalSession', c.session_id || '--');
    set('#dbgCalModel', c.model_id || '--');
    set('#dbgCalStatusRaw', s);
    set('#dbgCalMinDur', minDur > 0 ? `${minDur.toFixed(0)} s` : '--');
    set('#dbgCalMinFrames', minFrames > 0 ? `${minFrames}` : '--');
    set('#dbgCalFrameCount', minFrames > 0 ? `${frames} / ${minFrames}` : `${frames}`);
    set('#dbgCalElapsed', minDur > 0 ? `${elapsed.toFixed(0)} / ${minDur.toFixed(0)} s` : `${elapsed.toFixed(0)} s`);
    set('#dbgCalRemaining', collecting ? `${remaining.toFixed(0)} s` : '--');
    set('#dbgCalFps', fps > 0 ? `${fps.toFixed(1)} /s` : '--');
    set('#dbgCalGrid', grid.grid
      ? `node ${grid.source_node_id} · ${grid.grid.n_subcarriers} sc · ${grid.status}`
      : '--');
    set('#dbgCalRuntime', grid.runtime_history_frames != null
      ? `${grid.runtime_history_frames}/${grid.runtime_history_capacity}`
      : '--');

    const persisted = c.persisted_calibration || {};
    const persistEl = this.container.querySelector('#dbgCalPersist');
    if (persistEl && persistEl.style) {
      if (persisted.stored) {
        const expires = persisted.expires_at_unix_ms || 0;
        const days = expires ? Math.max(0, (expires - Date.now()) / 86400000) : 0;
        persistEl.textContent = `explicit-v1.json · 剩余 ${days.toFixed(1)} 天`;
        persistEl.style.color = days > 0 ? '#6b8' : '#c66';
        persistEl.title = expires ? new Date(expires).toLocaleString() : '';
      } else {
        persistEl.textContent = '未写入（Stop 成功后自动保存）';
        persistEl.style.color = '#667';
        persistEl.title = '';
      }
    }

    const bar = this.container.querySelector('#dbgCalBar');
    if (bar && bar.style) {
      bar.style.width = pct.toFixed(1) + '%';
      bar.style.background = stale ? '#e74c3c' : ready ? '#2ecc71' : '#f39c12';
    }
    const barText = this.container.querySelector('#dbgCalBarText');
    if (barText) {
      if (stale) {
        barText.textContent = `⚠ 已 ${(grid.latest_seen_ms / 1000).toFixed(0)} s 未收到 CSI 帧（grid stale）— 检查 UDP / 板子供电`;
      } else if (ready) {
        barText.textContent = `✓ 已达标（${frames} 帧，${elapsed.toFixed(0)} s）— 可以 Stop`;
      } else if (collecting) {
        barText.textContent = `采集中 ${pct.toFixed(0)}% · 剩余约 ${remaining.toFixed(0)} s · ${fps.toFixed(1)} 帧/s`;
      } else {
        barText.textContent = '';
      }
    }

    // Store identity for stop
    this._pendingStop = c.session_id ? {
      boot_epoch: c.boot_epoch,
      session_id: c.session_id,
      binding_digest: c.binding_digest,
      source_node_ids: c.source_node_ids || [1],
    } : null;

    // Stop is valid while a session is still collecting (status is
    // "uncalibrated" before the first frame, "collecting" after).
    const stopBtn = this.container.querySelector('#dbgCalStopBtn');
    if (stopBtn) stopBtn.disabled = !collecting;
  }

  // ── Sensing snapshot ─────────────────────────────────────────────────────

  _renderSensing() {
    const s = this._sensingState;
    if (!s) return;
    const set = (id, txt) => {
      const el = this.container.querySelector(id);
      if (el) el.textContent = txt;
    };
    set('#dbgTick', s.tick ?? '--');
    set('#dbgAge', s.age_ms != null ? s.age_ms + ' ms' : '--');
    set('#dbgSource', s.source || '--');
    set('#dbgPersonCount', s.estimated_persons ?? '--');
    set('#dbgPresence', s.classification?.presence ? 'true' : 'false');
    set('#dbgMotion', s.classification?.motion_level || '--');
    set('#dbgConfidence', s.classification?.confidence != null
      ? Number(s.classification.confidence).toFixed(2) : '--');
    set('#dbgNodes', Array.isArray(s.nodes) ? s.nodes.length : '--');
  }

  // ── Raw UDP packets (decoded) ───────────────────────────────────────────
  // The ESP32 emits several packet types over UDP. This panel shows the
  // server's decoded view of every admitted datagram (magic, size, and the
  // fields a human can read) so the wire format can be debugged without a
  // packet capture. Newest first; the server keeps the last 200.

  _startRawPackets() {
    this._stopRawPackets();
    this._rawPktPaused = false;
    this._pollRawPackets();
    this._rawPktTimer = setInterval(() => this._pollRawPackets(), 1000);
  }

  _stopRawPackets() {
    if (this._rawPktTimer) {
      clearInterval(this._rawPktTimer);
      this._rawPktTimer = null;
    }
  }

  async _pollRawPackets() {
    // The pause button freezes only the decoded-packet table below. The
    // fetch and the derived 0xC5110002 vitals points must keep running so
    // the other charts and the Edge Vitals card stay live.
    const statusEl = this.container.querySelector('#dbgRawPktStatus');
    try {
      const res = await fetch('/api/v1/debug/raw-packets', { cache: 'no-store' });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      this._rawPktState = await res.json();

      // 0xC5110002 path: the ESP32 edge pipeline's own BR/HR estimate, read
      // from the decoded raw-packet log (the gated /api/v1/edge-vitals
      // endpoint abstains without a fresh calibration). Dedupe by the
      // packet's own timestamp so ~1 Hz packets yield 1 Hz curve points.
      const edge = (this._rawPktState.by_magic || {})['0xC5110002']?.[0];
      if (edge && !edge.redacted) {
        const d = edge.decoded || {};
        if (d.timestamp_ms !== this._lastEdgeTs) {
          this._lastEdgeTs = d.timestamp_ms;
          this._edgeState = { t: edge.received_at_ms, ...d };
          this._pushPt(this._br2, edge.received_at_ms, d.breathing_rate_bpm);
          // The raw-packet decoder names the field `heart_rate_bpm`; accept
          // the struct spelling `heartrate_bpm` too, just in case.
          this._pushPt(this._hr2, edge.received_at_ms,
            d.heart_rate_bpm ?? d.heartrate_bpm);
        }
      }

      if (!this._rawPktPaused) this._renderRawPackets();
    } catch (err) {
      if (statusEl) {
        statusEl.textContent = `unavailable (${err.message}) — server rebuild pending?`;
      }
    }
  }

  _renderRawPackets() {
    const data = this._rawPktState;
    if (!data) return;
    const now = Date.now();
    const countersEl = this.container.querySelector('#dbgRawPktCounters');
    const statusEl = this.container.querySelector('#dbgRawPktStatus');
    const bodyEl = this.container.querySelector('#dbgRawPktBody');
    if (!countersEl || !bodyEl) return;

    // Merge the per-magic buckets for the selected types, then sort oldest →
    // newest. Each type keeps its own newest-N bucket, so the 1 Hz vitals
    // packets stay inspectable while the ~30 Hz raw-CSI stream floods the
    // global ring. The merged list is capped at the global capacity.
    const allEntries = data.entries || [];
    const selected = this._rawPktSelected;
    const byMagic = data.by_magic || {};
    let entries = [];
    if (selected.size) {
      const merged = [];
      for (const m of selected) {
        const bucket = byMagic[m];
        if (bucket) merged.push(...bucket);
        else merged.push(...allEntries.filter(e => e.magic === m));
      }
      merged.sort((a, b) => a.received_at_ms - b.received_at_ms);
      entries = merged.length > data.capacity
        ? merged.slice(merged.length - data.capacity)
        : merged;
    }

    if (statusEl) {
      const clock = new Date().toTimeString().slice(0, 8);
      const sel = selected.size ? [...selected].join(', ') : 'none';
      statusEl.textContent =
        `${entries.length} shown · selected: ${sel} · ${clock}` +
        (data.privacy_mode ? ' · privacy: redacted' : '');
    }

    // Per-magic counters, with the requested magics pinned to the front.
    const order = ['0xC5110001', '0xC5110002', '0xC5110003', '0xC5110005',
                   '0xC5110004', '0xC5110007', '0xC5110006', '0xC511A110', '—'];
    const counters = data.counters || {};
    const keys = order.filter(k => counters[k])
      .concat(Object.keys(counters).filter(k => !order.includes(k)));
    countersEl.innerHTML = keys.map(k => {
      const c = counters[k];
      const age = Math.max(0, now - (c.last_at_ms || 0));
      const ageStr = age < 1000 ? `${age}ms`
        : age < 60000 ? `${(age / 1000).toFixed(1)}s`
        : `${(age / 60000).toFixed(1)}m`;
      const cls = age < 5000 ? 'dbg-badge-ok'
        : age < 30000 ? 'dbg-badge-warn' : 'dbg-badge-info';
      return `<span class="dbg-badge ${cls}" style="font-size:10px;padding:3px 8px;" title="${k}">
        ${k} <b>${c.count}</b> · ${ageStr} · ${c.last_size}B
      </span>`;
    }).join('') || '<span style="color:#667;font-size:11px;">no counters</span>';

    // One toggle button per packet type. The canonical ESP32 magics always
    // get a button (even before the first packet of that type arrives), plus
    // any other magic the server has observed. Rebuild only when the set
    // changes, so clicking a button isn't clobbered by the 1 s poll; the
    // selected state is then refreshed in place.
    const magicsEl = this.container.querySelector('#dbgRawPktMagics');
    if (magicsEl) {
      const alwaysShow = ['0xC5110001', '0xC5110002', '0xC5110003', '0xC5110004',
                          '0xC5110005', '0xC5110007'];
      const magicList = [...new Set([...alwaysShow, ...keys.filter(k => k !== '—')])];
      const sig = magicList.join(',');
      if (magicsEl.dataset.sig !== sig) {
        magicsEl.innerHTML = magicList.map(k =>
          `<button type="button" data-magic="${k}" class="dbg-badge dbg-badge-info"
             style="cursor:pointer;border:0;font-size:11px;padding:4px 10px;">${k}</button>`
        ).join('');
        magicsEl.dataset.sig = sig;
      }
      magicsEl.querySelectorAll('button[data-magic]').forEach(btn => {
        const on = selected.has(btn.dataset.magic);
        btn.className = 'dbg-badge ' + (on ? 'dbg-badge-ok' : 'dbg-badge-info');
        btn.textContent = `${on ? '✓ ' : ''}${btn.dataset.magic}`;
      });
    }

    if (!entries.length) {
      const msg = selected.size
        ? 'No packets for the selected type(s) yet…'
        : 'No packet types selected — click a magic button above.';
      bodyEl.innerHTML = `<tr><td colspan="5" style="color:#555;padding:8px;">${msg}</td></tr>`;
      return;
    }

    // The panel re-renders every second. Remember which rows the user has
    // expanded so the innerHTML replacement doesn't collapse them.
    const openKeys = new Set();
    bodyEl.querySelectorAll('details[data-key][open]').forEach(el => {
      openKeys.add(el.dataset.key);
    });

    // Rows are oldest → newest, so keep the view pinned to the bottom unless
    // the user has scrolled up to inspect older packets.
    const scroller = this.container.querySelector('#dbgRawPktScroll');
    const stick = !this._rawPktScrollInit || !scroller ||
      (scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 48);

    bodyEl.innerHTML = entries.map(e => {
      const t = new Date(e.received_at_ms).toTimeString().slice(0, 8);
      const age = Math.max(0, now - e.received_at_ms);
      const ageStr = age < 1000 ? `${age}ms` : `${(age / 1000).toFixed(1)}s`;
      const kindCls = e.parsed ? 'dbg-ok' : 'dbg-bad';
      const summary = this._shortDecode(e);
      const json = this._escapeHtml(JSON.stringify(e.decoded, null, 2));
      const key = this._rawPktKey(e);
      const open = openKeys.has(key) ? ' open' : '';
      return `<tr style="border-bottom:1px solid #1a1a2a;">
        <td style="padding:3px 6px;white-space:nowrap;" title="${ageStr} ago">${t}</td>
        <td style="padding:3px 6px;white-space:nowrap;font-family:monospace;">${e.magic}</td>
        <td style="padding:3px 6px;white-space:nowrap;" class="${kindCls}">${e.kind}</td>
        <td style="padding:3px 6px;text-align:right;">${e.size}</td>
        <td style="padding:3px 6px;">
          <details data-key="${key}"${open}>
            <summary style="cursor:pointer;color:#cfd;">${summary}</summary>
            <pre style="margin:6px 0;padding:8px;background:#0d0d1a;border-radius:4px;max-height:220px;overflow:auto;font-size:10px;">${json}</pre>
          </details>
        </td>
      </tr>`;
    }).join('');

    if (scroller && stick) {
      scroller.scrollTop = scroller.scrollHeight;
      this._rawPktScrollInit = true;
    }
  }

  _rawPktKey(e) {
    const d = e.decoded || {};
    const seq = d.sequence ?? d.seq ?? '';
    return `${e.magic}:${seq}:${e.received_at_ms}`;
  }

  _shortDecode(e) {
    if (e.redacted) return 'redacted (privacy mode)';
    const d = e.decoded || {};
    const n = (v, nd = 1) => (v == null || Number.isNaN(Number(v)))
      ? '--' : Number(v).toFixed(nd);
    switch (e.magic) {
      case '0xC5110001':
        return `node=${d.node_id} subs=${d.n_subcarriers} seq=${d.sequence} rssi=${d.rssi_dbm}dBm ch=${d.channel_freq_mhz}MHz iq=${d.iq_bytes}B nz=${d.iq_nonzero_pairs ?? '--'}/${d.iq_pairs ?? '--'}`;
      case '0xC5110002':
        return `node=${d.node_id} BR=${n(d.breathing_rate_bpm)} HR=${n(d.heart_rate_bpm)} pres=${d.presence ? '✓' : '✗'} fall=${d.fall_detected ? '⚠' : '·'} motion=${d.motion ? '✓' : '✗'} persons=${d.n_persons} q=${n(d.presence_score, 2)}`;
      case '0xC5110003': {
        const eng = d.features_engineering || {};
        return `node=${d.node_id} seq=${d.seq} BR=${n(eng.breathing_rate_bpm)} HR=${n(eng.heart_rate_bpm)} pres=${n(eng.presence_score)} motion=${n(eng.motion_energy)} persons=${n(eng.person_count)} fall=${n(eng.fall_risk, 0)} rssi=${n(eng.rssi_dbm)}dBm`;
      }
      case '0xC5110004':
        return `node=${d.node_id} BR=${n(d.breathing_rate_bpm)} HR=${n(d.heartrate_bpm)} fusion=${d.fusion_confidence} mmwave(hr=${n(d.mmwave_hr_bpm)},br=${n(d.mmwave_br_bpm)},d=${n(d.mmwave_distance_cm, 0)}cm,targets=${d.mmwave_targets})`;
      case '0xC5110005':
        return `node=${d.node_id} ch=${d.channel} iq=${d.original_iq_bytes}B→${d.compressed_bytes}B (${n((d.compression_ratio || 0) * 100, 0)}%)`;
      case '0xC5110007':
        return `WASM output · ${e.size}B`;
      default:
        return `size=${e.size}B`;
    }
  }

  _escapeHtml(s) {
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');
  }

  // ── CSI / RF chart ──────────────────────────────────────────────────────
  // A faster 1 s poll captures the CSI-derived RF features from
  // /api/v1/sensing/latest and draws a live line chart. Raw per-subcarrier
  // amplitude is stripped from the live ESP32 publish for privacy, so when a
  // node does carry an amplitude vector we also render the subcarrier
  // spectrum; otherwise we chart the scalar features that are always present.

  _startChart() {
    this._stopChart();
    this._pollChart();
    this._chartTimer = setInterval(() => this._pollChart(), 1000);
  }

  _stopChart() {
    if (this._chartTimer) {
      clearInterval(this._chartTimer);
      this._chartTimer = null;
    }
  }

  async _pollChart() {
    try {
      const [sR, vR] = await Promise.allSettled([
        fetch('/api/v1/sensing/latest', { cache: 'no-store' }).then(r => r.json()),
        fetch('/api/v1/vital-signs', { cache: 'no-store' }).then(r => r.json()),
      ]);
      if (sR.status === 'fulfilled') {
        const s = sR.value;
        this._sensingState = s;
        if (s && s.status !== 'stale' && s.features) {
          this._recordRf(s);
        }
      }
      if (vR.status === 'fulfilled') {
        // 0xC5110001 path: the server CSI estimator's ungated vitals. In
        // privacy mode these are redacted (null), so nothing is recorded.
        const raw = vR.value?.raw_vital_signs || {};
        const now = Date.now();
        this._pushPt(this._br1, now, raw.breathing_rate_bpm);
        this._pushPt(this._hr1, now, raw.heart_rate_bpm);
      }
      this._renderChart();
      this._renderSensing();
      this._renderEdge();
    } catch { /* transient poll failure — keep last sample */ }
  }

  _pushPt(arr, t, v) {
    const x = Number(v);
    if (t == null || v == null || !isFinite(x)) return;
    arr.push({ t, v: x });
    const cutoff = Date.now() - this._rfWindowMs - 5000;
    while (arr.length && arr[0].t < cutoff) arr.shift();
    if (arr.length > 400) arr.shift();
  }

  _recordRf(s) {
    const f = s.features || {};
    const node = (s.nodes || [])[0] || {};
    this._rf.unshift({
      t: Date.now(),
      rssi: f.mean_rssi,
      variance: f.variance,
      dominant: f.dominant_freq_hz,
      breath: f.breathing_band_power,
      motion: f.motion_band_power,
    });
    if (this._rf.length > this._maxRf) this._rf.length = this._maxRf;
    if (Array.isArray(node.amplitude) && node.amplitude.length) {
      this._lastSpectrum = node.amplitude;
      this._wf.push({ t: Date.now(), amps: node.amplitude.slice() });
      if (this._wf.length > this._maxWf) this._wf.shift();
    }
  }

  _renderChart() {
    const note = this.container.querySelector('#dbgRfNote');
    const rows = this._rf;
    if (note) {
      note.textContent = rows.length
        ? `${rows.length} sample(s) · newest ${new Date(rows[0].t).toTimeString().slice(0, 8)} · window: last 2 min`
        : 'No live samples yet (ESP32 offline or stale).';
    }

    // One chart per series so each gets its own honest y-axis (with tick
    // labels) instead of sharing a unitless normalised axis.
    const rfChart = (id, key, opts) => this._drawTimeChart(
      this.container.querySelector(id),
      [{ color: opts.color, label: opts.label, points: rows.map(r => ({ t: r.t, v: r[key] })) }],
      opts,
    );
    rfChart('#dbgChRssi', 'rssi',
      { color: '#60a5fa', label: 'RSSI', unit: ' dBm' });
    rfChart('#dbgChVariance', 'variance',
      { color: '#f472b6', label: 'Variance', unit: '', minZero: true });
    rfChart('#dbgChDominant', 'dominant',
      { color: '#4ade80', label: 'Dominant freq (breath)', unit: ' Hz', minZero: true });
    rfChart('#dbgChBreath', 'breath',
      { color: '#fbbf24', label: 'Breath band power', unit: '', minZero: true, decimals: 3 });
    rfChart('#dbgChMotion', 'motion',
      { color: '#a78bfa', label: 'Motion band power', unit: '', minZero: true, decimals: 3 });

    // Breathing / heart-rate trends — one chart per magic so each series gets
    // its own y-scale. Sharing an axis hid the 0xC5110002 HR: the edge value
    // only moves ~±1.5 bpm around 80, which is invisible next to the server
    // estimate's 90-120 bpm swings.
    const vChart = (id, color, label, points, empty) => this._drawTimeChart(
      this.container.querySelector(id),
      [{ color, label, points }],
      { unit: ' bpm', decimals: 1, showRange: true, emptyText: empty },
    );
    vChart('#dbgChBr1', '#60a5fa', 'Breathing · 0xC5110001 server', this._br1,
      'Waiting for server breathing samples…');
    vChart('#dbgChBr2', '#fb923c', 'Breathing · 0xC5110002 edge', this._br2,
      'Waiting for edge breathing packets…');
    vChart('#dbgChHr1', '#60a5fa', 'Heart rate · 0xC5110001 server', this._hr1,
      'Waiting for server heart-rate samples…');
    vChart('#dbgChHr2', '#fb923c', 'Heart rate · 0xC5110002 edge', this._hr2,
      'Waiting for edge heart-rate packets…');

    // Subcarrier spectrum (when a raw amplitude vector is exposed).
    const spec = this.container.querySelector('#dbgSpectrum');
    if (spec) {
      const amps = this._lastSpectrum;
      const { ctx: sctx, w: sw, h: sh } = this._fitCanvas(spec, 110);
      sctx.clearRect(0, 0, sw, sh);
      if (!amps || !amps.length) {
        sctx.fillStyle = '#556';
        sctx.font = '12px sans-serif';
        sctx.fillText('Per-subcarrier amplitude not exposed on live ESP32 publish (privacy).', 8, sh / 2);
      } else {
        const bw = sw / amps.length;
        const amax = Math.max(...amps) || 1;
        sctx.fillStyle = '#38bdf8';
        amps.forEach((a, i) => {
          const bh = (a / amax) * (sh - 4);
          sctx.fillRect(i * bw, sh - bh, Math.max(1, bw - 0.5), bh);
        });
      }
    }

    this._renderWaterfall();
    this._renderBaselineCompare();
  }

  // Match the canvas backing store to its CSS box × devicePixelRatio and draw
  // in CSS pixels. Without this the buffer (1600 px wide) gets squeezed into a
  // ~700 px grid cell, shrinking 11 px text to ~5 px and blurring it.
  _fitCanvas(cv, fallbackH) {
    const rawDpr = (typeof window !== 'undefined' && window.devicePixelRatio) || 1;
    const dpr = Math.min(2, rawDpr); // cap at 2x to bound canvas memory
    const w = cv.clientWidth || 800;
    const h = cv.clientHeight || fallbackH || 150;
    const bw = Math.max(1, Math.round(w * dpr));
    const bh = Math.max(1, Math.round(h * dpr));
    if (cv.width !== bw || cv.height !== bh) {
      cv.width = bw;
      cv.height = bh;
    }
    const ctx = cv.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    return { ctx, w, h };
  }

  // ── Subcarrier waterfall (time × subcarrier) ──────────────────────────────

  _zColor(z) {
    const t = Math.max(-2, Math.min(2, z));
    const u = Math.min(1, Math.abs(t) / 2);
    const hue = t >= 0 ? 45 : 210;
    return `hsl(${hue} 85% ${8 + 47 * u}%)`;
  }

  _renderWaterfall() {
    const cv = this.container.querySelector('#dbgWaterfall');
    if (!cv) return;
    const { ctx, w, h } = this._fitCanvas(cv, 220);
    ctx.clearRect(0, 0, w, h);
    const note = this.container.querySelector('#dbgWfNote');
    const cols = this._wf;
    const padL = 46, padR = 14, padT = 26, padB = 26;
    const plotW = w - padL - padR, plotH = h - padT - padB;
    ctx.font = '12px sans-serif';

    if (!cols.length || !cols[0].amps || !cols[0].amps.length) {
      ctx.fillStyle = '#556';
      ctx.fillText('Waiting for per-subcarrier amplitude snapshots…', padL, padT + plotH / 2);
      if (note) note.textContent = 'no data';
      return;
    }

    const nsc = cols[0].amps.length;
    // Per-subcarrier mean/std over the window: the waterfall shows how each
    // bin deviates from its own average, so static multipath shape does not
    // swamp the breathing/motion modulation.
    const mean = new Float64Array(nsc);
    const std = new Float64Array(nsc);
    for (let s = 0; s < nsc; s++) {
      let m = 0, n = 0;
      for (const c of cols) { const v = c.amps[s]; if (isFinite(v)) { m += v; n++; } }
      m = n ? m / n : 0;
      let ss = 0;
      for (const c of cols) { const v = c.amps[s]; if (isFinite(v)) ss += (v - m) * (v - m); }
      mean[s] = m;
      std[s] = n > 1 ? Math.sqrt(ss / n) : 0;
    }

    const cw = plotW / cols.length;
    const rh = plotH / nsc;
    for (let ci = 0; ci < cols.length; ci++) {
      const col = cols[ci];
      const x = padL + ci * cw;
      for (let s = 0; s < nsc; s++) {
        const v = col.amps[s];
        const y = padT + plotH - (s + 1) * rh; // subcarrier 0 at the bottom
        let color = '#101018';
        if (isFinite(v) && std[s] > 1e-9) color = this._zColor((v - mean[s]) / std[s]);
        ctx.fillStyle = color;
        ctx.fillRect(x, y, Math.max(1, cw - 0.4), Math.max(1, rh));
      }
    }

    // Y ticks: subcarrier index every 8 bins.
    ctx.textAlign = 'right';
    for (let s = 0; s < nsc; s += 8) {
      const y = padT + plotH - (s + 0.5) * rh;
      ctx.fillStyle = '#778';
      ctx.fillText(String(s), padL - 6, y + 4);
    }
    // X ticks: time, oldest → newest.
    const t0 = cols[0].t, t1 = cols[cols.length - 1].t;
    for (let i = 0; i <= 4; i++) {
      const t = t0 + ((t1 - t0) * i) / 4;
      const x = padL + (plotW * i) / 4;
      ctx.textAlign = i === 0 ? 'left' : i === 4 ? 'right' : 'center';
      ctx.fillStyle = '#778';
      ctx.fillText(new Date(t).toTimeString().slice(0, 8), x, h - 8);
    }
    ctx.textAlign = 'left';

    if (note) {
      note.textContent = `${cols.length} snapshot(s) · ${nsc} subcarriers · ` +
        'color = per-subcarrier z-score (blue −2σ · dark 0 · yellow +2σ)';
    }
  }

  // ── Baseline comparison (amplitude vs subcarrier index) ──────────────────

  _loadBaseline() {
    try {
      const raw = localStorage.getItem('ruview.debug.baseline.v1');
      if (!raw) return null;
      const b = JSON.parse(raw);
      if (b && Array.isArray(b.amps) && b.amps.length) return b;
    } catch { /* no storage / corrupt entry */ }
    return null;
  }

  _captureBaseline() {
    const amps = this._lastSpectrum;
    if (!amps || !amps.length) return;
    this._baseline = { t: Date.now(), amps: amps.slice() };
    try {
      localStorage.setItem('ruview.debug.baseline.v1', JSON.stringify(this._baseline));
    } catch { /* storage full or blocked */ }
    this._renderBaselineCompare();
  }

  _clearBaseline() {
    this._baseline = null;
    try { localStorage.removeItem('ruview.debug.baseline.v1'); } catch { /* ignore */ }
    this._renderBaselineCompare();
  }

  _renderBaselineCompare() {
    const cv = this.container.querySelector('#dbgBaselineCmp');
    if (!cv) return;
    const note = this.container.querySelector('#dbgBaselineNote');
    const cur = this._lastSpectrum || [];
    const base = (this._baseline && this._baseline.amps) || [];
    const n = Math.max(cur.length, base.length);
    if (!n) {
      this._drawIndexChart(cv, [], { emptyText: 'Waiting for CSI amplitude…' });
      if (note) note.textContent = 'no data';
      return;
    }
    let rms = null;
    if (cur.length && base.length) {
      const m = Math.min(cur.length, base.length);
      let ss = 0, c = 0;
      for (let i = 0; i < m; i++) {
        if (isFinite(cur[i]) && isFinite(base[i])) {
          const d = cur[i] - base[i];
          ss += d * d;
          c++;
        }
      }
      if (c) rms = Math.sqrt(ss / c);
    }
    this._drawIndexChart(cv, [
      { color: '#9ca3af', label: 'baseline', points: base.map((v, i) => ({ i, v })), width: 1.4 },
      { color: '#60a5fa', label: 'current', points: cur.map((v, i) => ({ i, v })), width: 1.8 },
    ], { count: n, decimals: 1, minZero: true });
    if (note) {
      note.textContent = this._baseline
        ? `baseline captured ${new Date(this._baseline.t).toTimeString().slice(0, 8)} · ` +
          `RMS Δ = ${rms != null ? rms.toFixed(2) : '--'} amplitude units (56 bins)`
        : 'No baseline captured yet — click "Capture baseline" in a quiet/empty room.';
    }
  }

  // Index-based chart (x = subcarrier index) with labelled axes.
  _drawIndexChart(cv, seriesList, opts = {}) {
    const { ctx, w, h } = this._fitCanvas(cv, 200);
    ctx.clearRect(0, 0, w, h);
    const padL = 56, padR = 14, padT = 34, padB = 28;
    const plotW = w - padL - padR, plotH = h - padT - padB;
    ctx.font = '12px sans-serif';
    const n = Math.max(2, opts.count || 0,
      ...seriesList.map(s => (s.points || []).length));
    const xAt = (i) => padL + (i / (n - 1)) * plotW;
    const vals = seriesList.flatMap(s => (s.points || []).map(p => p.v))
      .filter(v => v != null && isFinite(v));
    if (!vals.length) {
      ctx.fillStyle = '#556';
      ctx.fillText(opts.emptyText || 'Waiting for data…', padL, padT + plotH / 2);
      return;
    }

    let lo = Math.min(...vals), hi = Math.max(...vals);
    if (hi - lo < 1e-9) { lo -= 1; hi += 1; }
    lo -= (hi - lo) * 0.12;
    hi += (hi - lo) * 0.12;
    if (opts.minZero) lo = Math.max(0, lo);
    const step = this._niceStep((hi - lo) / 4);
    const yDec = Math.max(0, Math.ceil(-Math.log10(step)));

    ctx.textAlign = 'right';
    for (let v = Math.floor(lo / step) * step; v <= hi + step * 1e-6; v += step) {
      const y = padT + plotH - ((v - lo) / (hi - lo)) * plotH;
      ctx.strokeStyle = '#1c1c2e';
      ctx.beginPath(); ctx.moveTo(padL, y); ctx.lineTo(w - padR, y); ctx.stroke();
      ctx.fillStyle = '#778';
      ctx.fillText(v.toFixed(yDec), padL - 6, y + 4);
    }

    ctx.textAlign = 'center';
    for (let i = 0; i < n; i += 8) {
      const x = xAt(i);
      ctx.strokeStyle = '#151522';
      ctx.beginPath(); ctx.moveTo(x, padT); ctx.lineTo(x, padT + plotH); ctx.stroke();
      ctx.fillStyle = '#778';
      ctx.fillText(String(i), x, h - 8);
    }
    ctx.textAlign = 'left';

    for (const s of seriesList) {
      const pts = (s.points || []).filter(p => p.v != null && isFinite(p.v));
      if (!pts.length) continue;
      ctx.strokeStyle = s.color;
      ctx.lineWidth = s.width || 1.6;
      ctx.beginPath();
      let started = false;
      for (const p of pts) {
        const x = xAt(p.i);
        const y = padT + plotH - ((p.v - lo) / (hi - lo)) * plotH;
        if (!started) { ctx.moveTo(x, y); started = true; } else { ctx.lineTo(x, y); }
      }
      ctx.stroke();
      ctx.lineWidth = 1;
    }

    const nd = opts.decimals ?? 1;
    let lx = padL;
    for (const s of seriesList) {
      const pts = s.points || [];
      const last = pts[pts.length - 1];
      const txt = `${s.label}: ${last ? last.v.toFixed(nd) : '--'}`;
      ctx.fillStyle = s.color;
      ctx.fillRect(lx, padT - 15, 10, 4);
      ctx.fillStyle = '#cfd';
      ctx.fillText(txt, lx + 14, padT - 11);
      lx += 14 + ctx.measureText(txt).width + 16;
    }
  }

  // One time-series chart with labelled y grid ticks, HH:MM:SS x ticks, and
  // an in-canvas legend showing each series' latest value.
  _drawTimeChart(cv, seriesList, opts = {}) {
    if (!cv) return;
    const { ctx, w, h } = this._fitCanvas(cv, 180);
    ctx.clearRect(0, 0, w, h);
    const unit = opts.unit || '';
    const now = Date.now();
    const winMs = this._rfWindowMs;
    const t0 = now - winMs;
    const padL = 66, padR = 14, padT = 34, padB = 28;
    const plotW = w - padL - padR, plotH = h - padT - padB;
    const xAt = (t) => padL + ((t - t0) / winMs) * plotW;
    ctx.font = '12px sans-serif';

    const series = seriesList.map(s => ({
      ...s,
      points: (s.points || []).filter(p => p.t >= t0 && p.v != null && isFinite(p.v)),
    }));
    const vals = series.flatMap(s => s.points.map(p => p.v));

    if (!vals.length) {
      ctx.fillStyle = '#556';
      ctx.fillText(opts.emptyText || 'Waiting for samples…', padL, padT + plotH / 2);
      return;
    }

    // Y range with headroom, clamped at zero for non-negative quantities.
    let lo = Math.min(...vals), hi = Math.max(...vals);
    if (hi - lo < 1e-9) { lo -= 1; hi += 1; }
    lo -= (hi - lo) * 0.12;
    hi += (hi - lo) * 0.12;
    if (opts.minZero) lo = Math.max(0, lo);
    const step = this._niceStep((hi - lo) / 4);
    const yDec = Math.max(0, Math.ceil(-Math.log10(step)));

    // Y gridlines + tick labels.
    ctx.textAlign = 'right';
    for (let v = Math.floor(lo / step) * step; v <= hi + step * 1e-6; v += step) {
      const y = padT + plotH - ((v - lo) / (hi - lo)) * plotH;
      ctx.strokeStyle = '#1c1c2e';
      ctx.beginPath();
      ctx.moveTo(padL, y);
      ctx.lineTo(w - padR, y);
      ctx.stroke();
      ctx.fillStyle = '#778';
      ctx.fillText(v.toFixed(yDec), padL - 6, y + 4);
    }

    // X ticks (time) + faint vertical gridlines.
    for (let i = 0; i <= 4; i++) {
      const t = t0 + (winMs * i) / 4;
      const x = xAt(t);
      ctx.strokeStyle = '#151522';
      ctx.beginPath();
      ctx.moveTo(x, padT);
      ctx.lineTo(x, padT + plotH);
      ctx.stroke();
      ctx.fillStyle = '#778';
      ctx.textAlign = i === 0 ? 'left' : i === 4 ? 'right' : 'center';
      ctx.fillText(new Date(t).toTimeString().slice(0, 8), x, h - 8);
    }
    ctx.textAlign = 'left';

    // Series lines.
    for (const s of series) {
      if (!s.points.length) continue;
      ctx.strokeStyle = s.color;
      ctx.lineWidth = 1.8;
      ctx.beginPath();
      let started = false;
      for (const p of s.points) {
        const x = xAt(p.t);
        const y = padT + plotH - ((p.v - lo) / (hi - lo)) * plotH;
        if (!started) { ctx.moveTo(x, y); started = true; } else { ctx.lineTo(x, y); }
      }
      ctx.stroke();
      if (s.points.length === 1) {
        const p = s.points[0];
        ctx.fillStyle = s.color;
        ctx.beginPath();
        ctx.arc(xAt(p.t), padT + plotH - ((p.v - lo) / (hi - lo)) * plotH, 2.5, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.lineWidth = 1;
    }

    // Legend with latest value per series (plus the window's min-max range so
    // small but real variation is visible even when the line looks flat).
    const nd = opts.decimals ?? 2;
    let lx = padL;
    for (const s of series) {
      const last = s.points[s.points.length - 1];
      let txt = `${s.label}: ${last ? last.v.toFixed(nd) : '--'}${unit}`;
      if (opts.showRange && s.points.length) {
        const vs = s.points.map(p => p.v);
        txt += `  · range ${Math.min(...vs).toFixed(nd)}–${Math.max(...vs).toFixed(nd)}${unit}`;
      }
      ctx.fillStyle = s.color;
      ctx.fillRect(lx, padT - 15, 10, 4);
      ctx.fillStyle = '#cfd';
      ctx.fillText(txt, lx + 14, padT - 11);
      lx += 14 + ctx.measureText(txt).width + 16;
    }
  }

  _niceStep(x) {
    if (!isFinite(x) || x <= 0) return 1;
    const base = Math.pow(10, Math.floor(Math.log10(x)));
    for (const m of [1, 2, 2.5, 5, 10]) {
      if (m * base >= x - 1e-12) return m * base;
    }
    return 10 * base;
  }

  // ── Edge vitals snapshot (magic 0xC5110002) ─────────────────────────────

  _renderEdge() {
    const e = this._edgeState;
    const set = (id, txt) => {
      const el = this.container.querySelector(id);
      if (el) el.textContent = txt;
    };
    if (!e) {
      for (const id of ['#dbgEdgeBR', '#dbgEdgeHR', '#dbgEdgePresence', '#dbgEdgeMotion',
                        '#dbgEdgePersons', '#dbgEdgeFall', '#dbgEdgeRssi', '#dbgEdgeAge']) {
        set(id, '--');
      }
      return;
    }
    set('#dbgEdgeBR', `${Number(e.breathing_rate_bpm ?? 0).toFixed(1)} bpm`);
    set('#dbgEdgeHR', `${Number(e.heart_rate_bpm ?? e.heartrate_bpm ?? 0).toFixed(1)} bpm`);
    set('#dbgEdgePresence', e.presence ? 'true' : 'false');
    set('#dbgEdgeMotion', e.motion ? 'true' : 'false');
    set('#dbgEdgePersons', `${e.n_persons ?? '--'}${e.person_count_valid ? '' : ' (invalid)'}`);
    set('#dbgEdgeFall', e.fall_detected ? '⚠ YES' : 'no');
    set('#dbgEdgeRssi', `${e.rssi_dbm ?? e.rssi ?? '--'} dBm`);
    set('#dbgEdgeAge', `${Math.max(0, Date.now() - e.t)} ms ago`);
  }

  // ── History export ──────────────────────────────────────────────────────

  _exportHistory(kind) {
    const rows = this._history.slice().reverse(); // oldest → newest
    let blob, name;
    if (kind === 'csv') {
      const head = ['time', 'breathing_bpm', 'heart_bpm', 'breath_conf', 'heart_conf', 'signal_quality', 'person_count', 'abstention_reason'];
      const esc = (v) => {
        if (v == null) return '';
        const s = String(v);
        return /[",\n]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s;
      };
      const lines = [head.join(',')].concat(rows.map(r =>
        [r.time, r.br, r.hr, r.brc, r.hrc, r.q, r.pc, r.reason].map(esc).join(',')));
      blob = new Blob([lines.join('\n') + '\n'], { type: 'text/csv' });
      name = 'ruview-raw-vitals.csv';
    } else {
      blob = new Blob([JSON.stringify(rows, null, 2) + '\n'], { type: 'application/json' });
      name = 'ruview-raw-vitals.json';
    }
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  // ── Calibration actions ──────────────────────────────────────────────────

  async _startCalibration() {
    const log = this.container.querySelector('#dbgCalLog');
    const nodeInput = this.container.querySelector('#dbgCalSourceNode');
    const nodeId = nodeInput ? nodeInput.value.trim() || '1' : '1';

    // Starting a new session replaces the current model. Ask before wiping a
    // usable calibration so a stray click cannot throw away 10+ minutes.
    const cur = this._calState || {};
    if (['fresh', 'stale', 'expired'].includes(cur.status)
        && typeof confirm === 'function'
        && !confirm('当前已有标定，重新开始会覆盖它（需要重新采集 ≥600 s / ≥1000 帧）。继续？')) {
      return;
    }

    if (log) log.textContent = 'Starting calibration…';

    try {
      const res = await fetch(`/api/v1/calibration/start?source_node_id=${nodeId}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          binding_digest: Array.from(crypto.getRandomValues(new Uint8Array(32)))
            .map(b => b.toString(16).padStart(2, '0')).join(''),
          source_node_ids: [Number(nodeId)],
        }),
      });
      const data = await res.json();
      if (log) {
        log.textContent = data.success
          ? `✓ Calibration started — model_id=${data.model_id}`
          : `✗ ${data.error || 'start failed'}`;
      }
      this._refresh(); // immediately poll status
    } catch (err) {
      if (log) log.textContent = '✗ ' + err.message;
    }
  }

  async _stopCalibration() {
    const log = this.container.querySelector('#dbgCalLog');
    const id = this._pendingStop;
    if (!id || !log) return;
    log.textContent = 'Stopping calibration…';

    try {
      const res = await fetch('/api/v1/calibration/stop', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(id),
      });
      const data = await res.json();
      if (log) {
        log.textContent = data.success
          ? `✓ Stopped — status=${data.status} frames=${data.frame_count} duration=${data.duration_s?.toFixed(0)}s`
          : `✗ ${data.error || 'stop failed'}`;
      }
      this._refresh();
    } catch (err) {
      if (log) log.textContent = '✗ ' + err.message;
    }
  }

  async _cancelCalibration() {
    const log = this.container.querySelector('#dbgCalLog');
    const id = this._pendingStop;
    if (!id || !log) return;
    log.textContent = 'Cancelling…';
    try {
      const res = await fetch('/api/v1/calibration/cancel', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(id),
      });
      const data = await res.json();
      if (log) log.textContent = data.success ? '✓ Cancelled' : '✗ ' + (data.error || 'cancel failed');
      this._refresh();
    } catch (err) {
      if (log) log.textContent = '✗ ' + err.message;
    }
  }

  // ── DOM ──────────────────────────────────────────────────────────────────

  _buildDOM() {
    this.container.innerHTML = `
      <div class="tab-header">
        <h2>Debug &amp; Diagnostics</h2>
        <p>Raw data collection, calibration controls, and full certification trace.</p>
      </div>

      <!-- Raw UDP packets (decoded) — ESP32 → server -->
      <div class="sensing-card" style="margin-bottom:16px;">
        <div class="sensing-card-title">
          Raw UDP Packets — decoded (ESP32 → server)
          <span style="float:right;display:flex;gap:6px;align-items:center;">
            <span id="dbgRawPktStatus" style="font-size:11px;color:#8899aa;">waiting…</span>
            <button id="dbgRawPktPause" class="btn btn-outline" title="只暂停下方数据表刷新；曲线和其他计算照常进行" style="font-size:11px;padding:4px 10px;">暂停表格</button>
          </span>
        </div>
        <div id="dbgRawPktMagics" style="display:flex;gap:6px;flex-wrap:wrap;margin:8px 0;"></div>
        <div id="dbgRawPktCounters" style="display:flex;gap:8px;flex-wrap:wrap;margin:8px 0;"></div>
        <div id="dbgRawPktScroll" style="overflow:auto;max-height:520px;border:1px solid #222;border-radius:6px;">
          <table style="width:100%;font-size:11px;border-collapse:collapse;">
            <thead>
              <tr style="position:sticky;top:0;background:#12121f;border-bottom:1px solid #333;color:#8899aa;text-align:left;">
                <th style="padding:4px 6px;">Time</th>
                <th style="padding:4px 6px;">Magic</th>
                <th style="padding:4px 6px;">Type</th>
                <th style="padding:4px 6px;">Size</th>
                <th style="padding:4px 6px;">Decoded</th>
              </tr>
            </thead>
            <tbody id="dbgRawPktBody">
              <tr><td colspan="5" style="color:#555;padding:8px;">Waiting for UDP packets…</td></tr>
            </tbody>
          </table>
        </div>
        <div style="font-size:10px;color:#667;margin-top:6px;">
          Oldest at top, newest at bottom. Click a magic button to show/hide that packet type; each type keeps its own newest 50. Click a row to expand full JSON (stays open while live).
        </div>
      </div>

      <!-- Vitals trends — one chart per metric per magic, independent y-scales -->
      <div class="sensing-card" style="margin-bottom:16px;">
        <div class="sensing-card-title">
          Vitals Trends — breathing &amp; heart rate (last 2 min, one chart per magic)
          <span class="dbg-src">0xC5110001 = server CSI estimate · 0xC5110002 = ESP32 edge vitals · each chart auto-scales to its own data</span>
        </div>
        <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(520px,1fr));gap:10px;">
          <canvas id="dbgChBr1" width="1600" height="180"
                  style="width:100%;height:180px;background:#0d0d1a;border-radius:6px;"></canvas>
          <canvas id="dbgChBr2" width="1600" height="180"
                  style="width:100%;height:180px;background:#0d0d1a;border-radius:6px;"></canvas>
          <canvas id="dbgChHr1" width="1600" height="180"
                  style="width:100%;height:180px;background:#0d0d1a;border-radius:6px;"></canvas>
          <canvas id="dbgChHr2" width="1600" height="180"
                  style="width:100%;height:180px;background:#0d0d1a;border-radius:6px;"></canvas>
        </div>
      </div>

      <!-- CSI / RF chart — full width, one chart per feature -->
      <div class="sensing-card" style="margin-bottom:16px;">
        <div class="sensing-card-title">
          CSI / RF Signal (live, 1 s/sample)
          <span class="dbg-src">source: 0xC5110001 CSI</span>
        </div>
        <div id="dbgRfNote" style="font-size:11px;color:#8899aa;margin:2px 0 8px;">Waiting…</div>
        <canvas id="dbgChRssi" width="1600" height="150"
                style="width:100%;height:150px;background:#0d0d1a;border-radius:6px;"></canvas>
        <canvas id="dbgChVariance" width="1600" height="150"
                style="width:100%;height:150px;margin-top:8px;background:#0d0d1a;border-radius:6px;"></canvas>
        <canvas id="dbgChDominant" width="1600" height="150"
                style="width:100%;height:150px;margin-top:8px;background:#0d0d1a;border-radius:6px;"></canvas>
        <canvas id="dbgChBreath" width="1600" height="150"
                style="width:100%;height:150px;margin-top:8px;background:#0d0d1a;border-radius:6px;"></canvas>
        <canvas id="dbgChMotion" width="1600" height="150"
                style="width:100%;height:150px;margin-top:8px;background:#0d0d1a;border-radius:6px;"></canvas>
        <div class="sensing-card-title" style="margin-top:14px;">
          Subcarrier Amplitude Spectrum
          <span class="dbg-src">source: 0xC5110001 CSI</span>
        </div>
        <canvas id="dbgSpectrum" width="1600" height="110"
                style="width:100%;height:110px;background:#0d0d1a;border-radius:6px;"></canvas>

        <div class="sensing-card-title" style="margin-top:14px;">
          Subcarrier Waterfall — dynamics over the last 2.5 min
          <span class="dbg-src">source: 0xC5110001 CSI</span>
        </div>
        <div id="dbgWfNote" style="font-size:11px;color:#8899aa;margin:2px 0 8px;">Waiting…</div>
        <canvas id="dbgWaterfall" width="1600" height="220"
                style="width:100%;height:220px;background:#0d0d1a;border-radius:6px;"></canvas>

        <div class="sensing-card-title" style="margin-top:14px;">
          Baseline Comparison — current vs captured baseline
          <span class="dbg-src">source: 0xC5110001 CSI</span>
          <span style="float:right;display:flex;gap:8px;">
            <button id="dbgBaselineBtn" class="btn btn-outline" style="font-size:12px;padding:4px 10px;">Capture baseline</button>
            <button id="dbgBaselineClearBtn" class="btn btn-outline" style="font-size:12px;padding:4px 10px;">Clear</button>
          </span>
        </div>
        <div id="dbgBaselineNote" style="font-size:11px;color:#8899aa;margin:2px 0 8px;">--</div>
        <canvas id="dbgBaselineCmp" width="1600" height="200"
                style="width:100%;height:200px;background:#0d0d1a;border-radius:6px;"></canvas>
      </div>

      <div class="sensing-layout" style="display:grid;grid-template-columns:1fr 1fr;gap:16px;">

        <!-- LEFT COLUMN ── Raw vitals + gates + certification trace -->
        <div style="display:flex;flex-direction:column;gap:16px;">

          <!-- Raw vitals snapshot -->
          <div class="sensing-card">
            <div class="sensing-card-title">
              Raw Vitals (ungated)
              <span class="dbg-src">source: 0xC5110001 CSI</span>
            </div>
            <div class="sensing-details">
              <div class="sensing-detail-row"><span>Breathing</span><span id="dbgRawBR">--</span></div>
              <div class="sensing-detail-row"><span>Heart Rate</span><span id="dbgRawHR">--</span></div>
              <div class="sensing-detail-row"><span>Breath Conf</span><span id="dbgRawBRC">--</span></div>
              <div class="sensing-detail-row"><span>Heart Conf</span><span id="dbgRawHRC">--</span></div>
              <div class="sensing-detail-row"><span>Signal Quality</span><span id="dbgRawQ">--</span></div>
            </div>
          </div>

          <!-- Edge vitals snapshot (ESP32 0xC5110002) -->
          <div class="sensing-card">
            <div class="sensing-card-title">
              Edge Vitals (ESP32)
              <span class="dbg-src">source: 0xC5110002 edge</span>
            </div>
            <div class="sensing-details">
              <div class="sensing-detail-row"><span>Breathing</span><span id="dbgEdgeBR">--</span></div>
              <div class="sensing-detail-row"><span>Heart Rate</span><span id="dbgEdgeHR">--</span></div>
              <div class="sensing-detail-row"><span>Presence</span><span id="dbgEdgePresence">--</span></div>
              <div class="sensing-detail-row"><span>Motion</span><span id="dbgEdgeMotion">--</span></div>
              <div class="sensing-detail-row"><span>Persons</span><span id="dbgEdgePersons">--</span></div>
              <div class="sensing-detail-row"><span>Fall</span><span id="dbgEdgeFall">--</span></div>
              <div class="sensing-detail-row"><span>RSSI</span><span id="dbgEdgeRssi">--</span></div>
              <div class="sensing-detail-row"><span>Packet Age</span><span id="dbgEdgeAge">--</span></div>
            </div>
          </div>

          <!-- Certified vitals snapshot -->
          <div class="sensing-card">
            <div class="sensing-card-title">
              Certified Vitals
              <span class="dbg-src">source: 0xC5110001 CSI</span>
              <span id="dbgAuthorityBadge" class="dbg-badge dbg-badge-info" style="float:right;">--</span>
            </div>
            <div class="sensing-details">
              <div class="sensing-detail-row"><span>Breathing</span><span id="dbgCertBR">--</span></div>
              <div class="sensing-detail-row"><span>Heart Rate</span><span id="dbgCertHR">--</span></div>
              <div class="sensing-detail-row"><span>Authority</span><span id="dbgAuth">--</span></div>
              <div class="sensing-detail-row"><span>Abstention Reason</span><span id="dbgReason">--</span></div>
            </div>
          </div>

          <!-- Gates -->
          <div class="sensing-card">
            <div class="sensing-card-title">
              Vitals Gates
              <span class="dbg-src">source: 0xC5110001 CSI</span>
            </div>
            <div class="sensing-details">
              <div class="sensing-detail-row"><span>Person Count (calibrated) == 1</span><span id="dbgGPersonCount">--</span></div>
              <div class="sensing-detail-row"><span>Signal Quality &ge; 0.40</span><span id="dbgGQual">--</span></div>
              <div class="sensing-detail-row"><span>Breath Conf &ge; 0.55</span><span id="dbgGBrc">--</span></div>
              <div class="sensing-detail-row"><span>Heart Conf &ge; 0.55</span><span id="dbgGHrc">--</span></div>
            </div>
          </div>

          <!-- Buffer fill -->
          <div class="sensing-card">
            <div class="sensing-card-title">
              Buffer Fill
              <span class="dbg-src">source: 0xC5110001 CSI</span>
            </div>
            <div class="sensing-details">
              <div class="sensing-detail-row"><span>Breath</span><span id="dbgBufBreath">--</span></div>
              <div class="sensing-detail-row"><span>Heart</span><span id="dbgBufHeart">--</span></div>
            </div>
          </div>

          <!-- Certification trace -->
          <div class="sensing-card">
            <div class="sensing-card-title">
              Certification Trace
              <span class="dbg-src">source: 0xC5110001 CSI</span>
            </div>
            <div id="dbgCertTrace"></div>
          </div>

          <!-- Fall detection (server-side edge equivalent) -->
          <div class="sensing-card" id="dbgFallCard">
            <div class="sensing-card-title">
              Fall Detection
              <span class="dbg-src">source: 0xC5110001 CSI · server edge #263</span>
              <span id="dbgFallBadge" class="dbg-badge dbg-badge-ok" style="float:right;">--</span>
            </div>
            <div class="sensing-details">
              <div class="sensing-detail-row"><span>Node</span><span id="dbgFallNode">--</span></div>
              <div class="sensing-detail-row"><span>Spike Score (RMS z)</span><span id="dbgFallAccel">--</span></div>
              <div class="sensing-detail-row"><span>Spike Threshold</span><span id="dbgFallThresh">--</span></div>
              <div class="sensing-detail-row"><span>Spike Consec</span><span id="dbgFallConsec">--</span></div>
              <div class="sensing-detail-row"><span>Still Score (mean |&Delta;s|)</span><span id="dbgFallStill">--</span></div>
              <div class="sensing-detail-row"><span>Pending</span><span id="dbgFallPending">--</span></div>
              <div class="sensing-detail-row"><span>Last Alert</span><span id="dbgFallLastAlert">--</span></div>
            </div>
            <canvas id="dbgFallChart" style="width:100%;margin-top:6px;border-radius:6px;"></canvas>
            <div style="font-size:10px;color:#667;margin-top:4px;">
              基线 z-score 尖峰 ≥ 阈值连续 5 帧 + 静止确认（4 s 平均 |Δs| <
              确认线，默认 9%，持续 12 s）→ 锁存 10 s。区分度来自静止时长：坐姿停顿只有
              2-5 s，跌倒后人不再起身。AGC 免疫：逐帧形状归一化。
            </div>
            <div style="display:flex;gap:6px;align-items:center;margin-top:8px;">
              <button id="dbgFallCapBtn" class="btn btn-primary" style="font-size:11px;padding:4px 10px;">开始捕捉</button>
              <button id="dbgFallCapClearBtn" class="btn btn-outline" style="font-size:11px;padding:4px 10px;">清除</button>
              <button id="dbgFallExportBtn" class="btn btn-outline" style="font-size:11px;padding:4px 10px;">导出</button>
              <button id="dbgFallImportBtn" class="btn btn-outline" style="font-size:11px;padding:4px 10px;">导入</button>
              <input type="file" id="dbgFallImportFile" accept=".json,application/json" style="display:none;">
              <span id="dbgFallCapInfo" style="font-size:10px;color:#667;">未捕捉 — 点「开始捕捉」录制波形（服务端逐帧）</span>
            </div>
            <canvas id="dbgFallCapChart" style="width:100%;height:258px;margin-top:6px;border-radius:6px;"></canvas>
          </div>
        </div>

        <!-- RIGHT COLUMN ── Sensing + history -->
        <div style="display:flex;flex-direction:column;gap:16px;">

          <!-- Sensing snapshot -->
          <div class="sensing-card">
            <div class="sensing-card-title">
              Sensing Snapshot
              <span class="dbg-src">source: 0xC5110001 CSI features</span>
            </div>
            <div class="sensing-details">
              <div class="sensing-detail-row"><span>Source</span><span id="dbgSource">--</span></div>
              <div class="sensing-detail-row"><span>Tick</span><span id="dbgTick">--</span></div>
              <div class="sensing-detail-row"><span>Age</span><span id="dbgAge">--</span></div>
              <div class="sensing-detail-row"><span>Est. Persons</span><span id="dbgPersonCount">--</span></div>
              <div class="sensing-detail-row"><span>Presence</span><span id="dbgPresence">--</span></div>
              <div class="sensing-detail-row"><span>Motion</span><span id="dbgMotion">--</span></div>
              <div class="sensing-detail-row"><span>Confidence</span><span id="dbgConfidence">--</span></div>
              <div class="sensing-detail-row"><span>Nodes</span><span id="dbgNodes">--</span></div>
            </div>
          </div>

          <!-- History table -->
          <div class="sensing-card" style="grid-column:1 / -1;">
            <div class="sensing-card-title">
              Raw Vitals History (last ${this._maxHistory} samples)
              <span class="dbg-src">source: 0xC5110001 CSI</span>
              <span style="float:right;display:flex;gap:6px;">
                <button id="dbgExportCsv" class="btn btn-outline" style="font-size:11px;padding:4px 10px;">导出 CSV</button>
                <button id="dbgExportJson" class="btn btn-outline" style="font-size:11px;padding:4px 10px;">导出 JSON</button>
              </span>
            </div>
            <div style="overflow-x:auto;">
              <table style="width:100%;font-size:11px;border-collapse:collapse;">
                <thead>
                  <tr style="border-bottom:1px solid #333;color:#8899aa;text-align:left;">
                    <th style="padding:4px 6px;">Time</th>
                    <th style="padding:4px 6px;">BR (bpm)</th>
                    <th style="padding:4px 6px;">HR (bpm)</th>
                    <th style="padding:4px 6px;">BrConf</th>
                    <th style="padding:4px 6px;">HrConf</th>
                    <th style="padding:4px 6px;">Quality</th>
                    <th style="padding:4px 6px;">PC</th>
                    <th style="padding:4px 6px;">Reason</th>
                  </tr>
                </thead>
                <tbody id="dbgHistoryBody">
                  <tr><td colspan="8" style="color:#555;padding:8px;">Collecting…</td></tr>
                </tbody>
              </table>
            </div>
          </div>

          <!-- Certified vitals history table -->
          <div class="sensing-card" style="grid-column:1 / -1;">
            <div class="sensing-card-title">
              Certified Vitals History (showing last 20 / kept ${this._maxCertHistory})
              <span class="dbg-src">source: 0xC5110001 CSI · explicit_calibration</span>
              <span style="float:right;display:flex;gap:6px;">
                <button id="dbgCertExportCsv" class="btn btn-outline" style="font-size:11px;padding:4px 10px;">导出 CSV</button>
                <button id="dbgCertExportJson" class="btn btn-outline" style="font-size:11px;padding:4px 10px;">导出 JSON</button>
              </span>
            </div>
            <div style="font-size:10px;color:#667;margin:2px 0 6px;">
              仅记录通过全部认证门控并实际发布的样本（authority = explicit_calibration）；未通过的留在上方 Raw Vitals History。
            </div>
            <div style="overflow-x:auto;">
              <table style="width:100%;font-size:11px;border-collapse:collapse;">
                <thead>
                  <tr style="border-bottom:1px solid #333;color:#8899aa;text-align:left;">
                    <th style="padding:4px 6px;">Time</th>
                    <th style="padding:4px 6px;">BR (bpm)</th>
                    <th style="padding:4px 6px;">HR (bpm)</th>
                    <th style="padding:4px 6px;">BrConf</th>
                    <th style="padding:4px 6px;">HrConf</th>
                    <th style="padding:4px 6px;">Quality</th>
                    <th style="padding:4px 6px;">PC</th>
                    <th style="padding:4px 6px;">Authority</th>
                    <th style="padding:4px 6px;">Tick</th>
                  </tr>
                </thead>
                <tbody id="dbgCertHistoryBody">
                  <tr><td colspan="9" style="color:#555;padding:8px;">Collecting…</td></tr>
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>

      <!-- Calibration controls — full width, last on the page -->
      <div class="sensing-card">
        <div class="sensing-card-title">
          Calibration
          <span id="dbgCalBadge" class="dbg-badge dbg-badge-info" style="float:right;">--</span>
        </div>
        <div style="display:flex;gap:8px;align-items:center;margin:8px 0;">
          <label style="font-size:11px;color:#8899aa;">Node ID:</label>
          <input id="dbgCalSourceNode" type="number" value="1" min="1" max="10"
                 style="width:50px;padding:4px;font-size:12px;background:#1a1a2e;color:#eee;border:1px solid #333;border-radius:4px;" />
          <button id="dbgCalStartBtn" class="btn btn-primary" style="font-size:12px;padding:6px 12px;">Start</button>
          <button id="dbgCalStopBtn" class="btn btn-danger" style="font-size:12px;padding:6px 12px;" disabled>Stop</button>
          <button id="dbgCalCancelBtn" class="btn btn-outline" style="font-size:12px;padding:6px 12px;">Cancel</button>
        </div>
        <div id="dbgCalLog" style="font-size:11px;color:#8899aa;margin:4px 0;min-height:16px;"></div>
        <div style="margin:6px 0 10px;">
          <div style="height:8px;background:#1a1a2e;border:1px solid #333;border-radius:4px;overflow:hidden;">
            <div id="dbgCalBar" style="height:100%;width:0%;background:#f39c12;transition:width .4s ease;"></div>
          </div>
          <div id="dbgCalBarText" style="font-size:11px;color:#8899aa;margin-top:4px;"></div>
        </div>
        <div class="sensing-details">
          <div class="sensing-detail-row"><span>Status</span><span id="dbgCalStatusRaw">--</span></div>
          <div class="sensing-detail-row"><span>Session ID</span><span id="dbgCalSession" style="font-size:10px;word-break:break-all;">--</span></div>
          <div class="sensing-detail-row"><span>Model ID</span><span id="dbgCalModel" style="font-size:10px;word-break:break-all;">--</span></div>
          <div class="sensing-detail-row"><span>Min Duration</span><span id="dbgCalMinDur">--</span></div>
          <div class="sensing-detail-row"><span>Min Frames</span><span id="dbgCalMinFrames">--</span></div>
          <div class="sensing-detail-row"><span>Collected</span><span id="dbgCalFrameCount">--</span></div>
          <div class="sensing-detail-row"><span>Elapsed</span><span id="dbgCalElapsed">--</span></div>
          <div class="sensing-detail-row"><span>Remaining</span><span id="dbgCalRemaining">--</span></div>
          <div class="sensing-detail-row"><span>Frame Rate</span><span id="dbgCalFps">--</span></div>
          <div class="sensing-detail-row"><span>Grid Binding</span><span id="dbgCalGrid">--</span></div>
          <div class="sensing-detail-row"><span>Runtime History</span><span id="dbgCalRuntime">--</span></div>
          <div class="sensing-detail-row"><span>Persisted</span><span id="dbgCalPersist">--</span></div>
        </div>
        <div style="font-size:10px;color:#667;margin-top:6px;">
          保持空房直到 Collected 与 Elapsed 都达标；两项都达标后 Stop 才会成功。
        </div>
      </div>
    `;

    // Wire up buttons
    this.container.querySelector('#dbgCalStartBtn')?.addEventListener('click', () => this._startCalibration());
    this.container.querySelector('#dbgCalStopBtn')?.addEventListener('click', () => this._stopCalibration());
    this.container.querySelector('#dbgCalCancelBtn')?.addEventListener('click', () => this._cancelCalibration());
    this.container.querySelector('#dbgExportCsv')?.addEventListener('click', () => this._exportHistory('csv'));
    this.container.querySelector('#dbgExportJson')?.addEventListener('click', () => this._exportHistory('json'));
    this.container.querySelector('#dbgCertExportCsv')?.addEventListener('click', () => this._exportCertHistory('csv'));
    this.container.querySelector('#dbgCertExportJson')?.addEventListener('click', () => this._exportCertHistory('json'));
    this.container.querySelector('#dbgFallCapBtn')?.addEventListener('click', () => this._toggleFallCapture());
    this.container.querySelector('#dbgFallCapClearBtn')?.addEventListener('click', () => this._clearFallCapture());
    this.container.querySelector('#dbgFallExportBtn')?.addEventListener('click', () => this._exportFallCapture());
    this.container.querySelector('#dbgFallImportBtn')?.addEventListener('click', () => this.container.querySelector('#dbgFallImportFile')?.click());
    this.container.querySelector('#dbgFallImportFile')?.addEventListener('change', (ev) => {
      const file = ev.target.files && ev.target.files[0];
      if (file) this._importFallCapture(file);
      ev.target.value = ''; // 允许重复导入同一个文件
    });
    this.container.querySelector('#dbgBaselineBtn')?.addEventListener('click', () => this._captureBaseline());
    this.container.querySelector('#dbgBaselineClearBtn')?.addEventListener('click', () => this._clearBaseline());
    this.container.querySelector('#dbgRawPktPause')?.addEventListener('click', (ev) => {
      this._rawPktPaused = !this._rawPktPaused;
      ev.currentTarget.textContent = this._rawPktPaused ? '继续表格' : '暂停表格';
      const st = this.container.querySelector('#dbgRawPktStatus');
      if (this._rawPktPaused) {
        if (st && !st.textContent.includes('表格已暂停')) {
          st.textContent += ' · 表格已暂停（曲线仍在更新）';
        }
      } else {
        // Resuming: paint the latest snapshot immediately instead of
        // waiting for the next 1 s poll.
        this._renderRawPackets();
      }
    });
    this.container.querySelector('#dbgRawPktMagics')?.addEventListener('click', (ev) => {
      const btn = ev.target.closest('button[data-magic]');
      if (!btn) return;
      const magic = btn.dataset.magic;
      if (this._rawPktSelected.has(magic)) this._rawPktSelected.delete(magic);
      else this._rawPktSelected.add(magic);
      this._renderRawPackets();
    });
  }
}
