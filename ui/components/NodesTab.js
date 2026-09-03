// Node management.
//
// Every value shown here is read from a live node through the server proxy.
// Nothing is simulated, interpolated, or carried over from a previous poll: a
// field the fleet has not reported renders as an em dash. That matters because
// plausible-looking numbers generated in the browser have been mistaken for
// sensing data in this project before.
//
// The server holds the fleet OTA key and forwards requests to nodes, so the
// browser never sees it. See ADR-351 for what that does and does not protect.

const NODE_ENDPOINT = '/api/v1/nodes';

// Parameters the firmware re-reads every cycle, so a change applies with no
// restart. Mirrors is_live() in config_api.c.
const LIVE_KEYS = ['led_mode', 'led_brightness'];

const LED_MODES = [
  { value: 0, label: 'Off' },
  { value: 1, label: 'Steady' },
  { value: 2, label: 'Flicker 40Hz' },
];

const FIELDS = [
  { key: 'node_id', label: 'Node ID', type: 'int' },
  { key: 'target_ip', label: 'Server IP', type: 'text' },
  { key: 'target_port', label: 'Server port', type: 'int' },
  { key: 'zone_name', label: 'Zone name', type: 'text' },
  { key: 'tdm_node_count', label: 'Fleet size', type: 'int' },
  { key: 'tdm_slot_index', label: 'TDM slot', type: 'int' },
  { key: 'beacon_period_ms', label: 'Beacon period ms', type: 'int' },
  { key: 'edge_tier', label: 'Edge tier', type: 'int' },
  { key: 'presence_thresh', label: 'Presence threshold', type: 'float' },
  { key: 'fall_thresh', label: 'Fall threshold', type: 'float' },
  { key: 'vital_window', label: 'Vital window', type: 'int' },
  { key: 'vital_interval_ms', label: 'Vital interval ms', type: 'int' },
  { key: 'top_k_count', label: 'Top-K subcarriers', type: 'int' },
  { key: 'power_duty', label: 'Power duty pct', type: 'int' },
  { key: 'swarm_heartbeat_sec', label: 'Swarm heartbeat s', type: 'int' },
  { key: 'swarm_ingest_sec', label: 'Swarm ingest s', type: 'int' },
  { key: 'wifi_ssid', label: 'WiFi SSID', type: 'text' },
  { key: 'csi_channel', label: 'CSI channel', type: 'int' },
  { key: 'channel_hop_count', label: 'Channel hop count', type: 'int' },
  { key: 'dwell_ms', label: 'Dwell ms', type: 'int' },
];

function dash(v) {
  return (v === null || v === undefined || v === '') ? '—' : v;
}

export class NodesTab {
  constructor(containerElement) {
    this.container = containerElement;
    this.nodes = [];
    this.detail = null;
    this.selected = null;
    this.timer = null;
    this.error = null;
  }

  init() {
    this.render();
    this.refresh();
  }

  activate() {
    if (!this.timer) {
      this.timer = setInterval(() => this.refresh(), 10000);
    }
    this.refresh();
  }

  deactivate() {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  async refresh() {
    try {
      const r = await fetch(NODE_ENDPOINT);
      if (!r.ok) {
        throw new Error('HTTP ' + r.status);
      }
      const data = await r.json();
      this.nodes = (data.nodes || []).slice().sort((a, b) => a.node_id - b.node_id);
      this.error = null;
    } catch (e) {
      // Report the failure rather than leaving stale rows that still look live.
      this.error = e.message;
      this.nodes = [];
    }
    this.renderTable();
  }

  async get(url) {
    try {
      const r = await fetch(url);
      const body = await r.json().catch(() => ({}));
      return { ok: r.ok, status: r.status, body: body };
    } catch (e) {
      return { ok: false, status: 0, body: { error: e.message } };
    }
  }

  async loadDetail(id) {
    this.selected = id;
    this.detail = null;
    this.renderDetail('Reading node ' + id + '...');
    const results = await Promise.all([
      this.get(NODE_ENDPOINT + '/' + id + '/config'),
      this.get(NODE_ENDPOINT + '/' + id + '/firmware'),
    ]);
    const cfg = results[0];
    const fw = results[1];
    this.detail = {
      id: id,
      config: cfg.ok ? (cfg.body.config || {}) : null,
      requiresTrial: cfg.ok ? (cfg.body.requires_trial || []) : [],
      trialPending: cfg.ok ? Boolean(cfg.body.trial_pending) : false,
      firmware: fw.ok ? fw.body : null,
      error: cfg.ok ? null : (cfg.body.error || ('HTTP ' + cfg.status)),
    };
    this.renderTable();
    this.renderDetail();
  }

  async push(id, payload) {
    const keys = Object.keys(payload);
    const trialKeys = (this.detail && this.detail.requiresTrial) || [];
    const isTrial = keys.some((k) => trialKeys.indexOf(k) !== -1);
    const allLive = keys.every((k) => LIVE_KEYS.indexOf(k) !== -1);
    try {
      const r = await fetch(NODE_ENDPOINT + '/' + id + '/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const body = await r.json().catch(() => ({}));
      if (!r.ok) {
        this.say('Rejected: ' + (body.error || body.message || ('HTTP ' + r.status)), 'err');
      } else if (body.trial) {
        this.say('Applied on trial. Node ' + id + ' is rebooting and reverts in '
          + body.trial_seconds + 's unless it rejoins. Do not push again.', 'warn');
      } else if (allLive) {
        this.say('Applied to node ' + id + ' immediately, no restart.', 'ok');
      } else {
        this.say('Written to node ' + id + '. It is rebooting to apply.', 'ok');
      }
    } catch (e) {
      // A lost reply on a trial push is not a failure: the node answers, then
      // reboots 3s later, and the write is already committed. Re-pushing at
      // that moment is the one genuinely harmful response.
      this.say(isTrial
        ? 'No reply from node ' + id + '. On a trial change this usually means '
          + 'it armed and rebooted. Wait for it to return, do not push again.'
        : 'Failed: ' + e.message, 'warn');
    }
    const self = this;
    setTimeout(() => self.loadDetail(id), isTrial ? 9000 : 1500);
  }

  say(msg, kind) {
    const el = this.container.querySelector('#nodes-status');
    if (!el) {
      return;
    }
    el.textContent = msg;
    el.className = 'nodes-status nodes-' + kind;
  }

  render() {
    this.container.innerHTML = ''
      + '<div class="nodes-tab">'
      + '<h2>Node Management</h2>'
      + '<p class="nodes-sub">Values are read live from each node. Nothing on '
      + 'this page is simulated.</p>'
      + '<div id="nodes-status" class="nodes-status"></div>'
      + '<div id="nodes-table"></div>'
      + '<div id="nodes-detail"></div>'
      + '</div>';
  }

  renderTable() {
    const el = this.container.querySelector('#nodes-table');
    if (!el) {
      return;
    }
    if (this.error) {
      el.innerHTML = '<p class="nodes-err">Could not reach the server: '
        + this.error + '</p>';
      return;
    }
    const rows = this.nodes.map((n) => ''
      + '<tr class="' + (n.node_id === this.selected ? 'sel' : '') + '">'
      + '<td>' + n.node_id + '</td>'
      + '<td>' + dash(n.ip) + '</td>'
      + '<td class="' + (n.status === 'active' ? 'ok' : 'warn') + '">' + n.status + '</td>'
      + '<td>' + dash(n.rssi_dbm) + '</td>'
      + '<td>' + dash(n.last_seen_ms) + ' ms</td>'
      + '<td><button class="nodes-manage" data-id="' + n.node_id + '">Manage</button></td>'
      + '</tr>').join('');
    el.innerHTML = ''
      + '<table class="nodes-table"><thead><tr>'
      + '<th>Node</th><th>Address</th><th>Status</th><th>RSSI</th>'
      + '<th>Last seen</th><th></th></tr></thead><tbody>'
      + (rows || '<tr><td colspan="6">No nodes reporting.</td></tr>')
      + '</tbody></table>';
    const self = this;
    el.querySelectorAll('.nodes-manage').forEach((b) => {
      b.addEventListener('click', () => self.loadDetail(Number(b.dataset.id)));
    });
  }

  renderDetail(placeholder) {
    const el = this.container.querySelector('#nodes-detail');
    if (!el) {
      return;
    }
    if (placeholder) {
      el.innerHTML = '<p>' + placeholder + '</p>';
      return;
    }
    const d = this.detail;
    if (!d) {
      el.innerHTML = '';
      return;
    }
    if (d.error) {
      el.innerHTML = '<p class="nodes-err">Node ' + d.id + ': ' + d.error + '</p>';
      return;
    }

    const fw = d.firmware || {};
    const rollback = fw.last_rollback
      ? '<p class="nodes-err">Last rollback: ' + fw.last_rollback + '</p>' : '';
    const pending = fw.pending_verify
      ? '<p class="nodes-warn">Firmware is on trial and not yet confirmed.</p>' : '';
    const trialPending = d.trialPending
      ? '<p class="nodes-warn">A config trial is pending. Changes are refused '
        + 'until it commits or reverts.</p>' : '';

    const rows = FIELDS.map((f) => {
      const v = d.config[f.key];
      const isTrial = d.requiresTrial.indexOf(f.key) !== -1;
      const isLive = LIVE_KEYS.indexOf(f.key) !== -1;
      let tag = '';
      if (isTrial) {
        tag = '<span class="tag trial">reboots, reverts if it cannot rejoin</span>';
      } else if (isLive) {
        tag = '<span class="tag live">applies instantly</span>';
      }
      const shown = (v === null || v === undefined) ? '' : v;
      return '<tr><td>' + f.label + ' ' + tag + '</td>'
        + '<td><input data-key="' + f.key + '" data-type="' + f.type + '" '
        + 'value="' + shown + '" placeholder="unset"></td></tr>';
    }).join('');

    const ledButtons = LED_MODES.map((m) => '<button class="led-btn'
      + (d.config.led_mode === m.value ? ' on' : '') + '" data-led="' + m.value
      + '">' + m.label + '</button>').join('');

    el.innerHTML = ''
      + '<div class="nodes-detail">'
      + '<h3>Node ' + d.id + '</h3>'
      + '<p>Firmware <strong>' + dash(fw.version) + '</strong> (' + dash(fw.date)
      + '), running <strong>' + dash(fw.running_partition) + '</strong></p>'
      + pending + rollback + trialPending
      + '<div class="nodes-led"><label>LED</label>' + ledButtons
      + '<label>Brightness</label>'
      + '<input id="led-bright" type="range" min="0" max="100" value="'
      + (d.config.led_brightness === null || d.config.led_brightness === undefined
          ? 100 : d.config.led_brightness) + '">'
      + '<span id="led-bright-val">' + dash(d.config.led_brightness) + '</span></div>'
      + '<table class="nodes-fields">' + rows + '</table>'
      + '<button id="nodes-save">Apply changed settings</button>'
      + '</div>';

    const self = this;
    el.querySelectorAll('.led-btn').forEach((b) => {
      b.addEventListener('click', () => self.push(d.id, { led_mode: Number(b.dataset.led) }));
    });
    const slider = el.querySelector('#led-bright');
    slider.addEventListener('input', () => {
      el.querySelector('#led-bright-val').textContent = slider.value;
    });
    slider.addEventListener('change', () => {
      self.push(d.id, { led_brightness: Number(slider.value) });
    });

    el.querySelector('#nodes-save').addEventListener('click', () => {
      const payload = {};
      el.querySelectorAll('.nodes-fields input').forEach((i) => {
        const key = i.dataset.key;
        const raw = i.value.trim();
        if (raw === '') {
          return;                      // never write a field the node has unset
        }
        const val = i.dataset.type === 'text' ? raw : Number(raw);
        if (i.dataset.type !== 'text' && Number.isNaN(val)) {
          return;
        }
        if (String(d.config[key]) !== String(val)) {
          payload[key] = val;          // send only what actually changed
        }
      });
      const keys = Object.keys(payload);
      if (keys.length === 0) {
        self.say('Nothing changed.', 'warn');
        return;
      }
      const risky = keys.filter((k) => d.requiresTrial.indexOf(k) !== -1);
      const msg = risky.length
        ? risky.join(', ') + ' affect how this node joins the network. It will '
          + 'reboot, and revert on its own if it cannot rejoin. Continue?'
        : 'Apply ' + keys.join(', ') + ' to node ' + d.id + '?';
      if (window.confirm(msg)) {
        self.push(d.id, payload);
      }
    });
  }
}
