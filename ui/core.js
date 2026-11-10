/**
 * RuView Physical Telemetry Frontend Engine
 * Strictly WCAG 2.2 AA compliant, zero WebGL/Three.js dependency.
 * Follows ElmasCore charter: Pure functions, low overhead, modular logic.
 */

(() => {
  'use strict';

  // COCO 17 Keypoint Skeletons (Bones between joint pairs)
  const SKELETON_PAIRS = [
    [0, 1], [0, 2], [1, 3], [2, 4],       // Head
    [5, 6], [5, 7], [7, 9], [6, 8], [8, 10], // Arms
    [5, 11], [6, 12], [11, 12],            // Torso
    [11, 13], [13, 15], [12, 14], [14, 16] // Legs
  ];

  const STATE = {
    connected: false,
    theme: 'dark',
    sparklineBuffer: new Array(100).fill(40),
    ws: null,
    reconnectTimer: null
  };

  function initTheme() {
    const saved = localStorage.getItem('ruview-theme');
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const initial = saved || (prefersDark ? 'dark' : 'light');
    setTheme(initial);

    const toggleBtn = document.getElementById('theme-toggle');
    if (toggleBtn) {
      toggleBtn.addEventListener('click', () => {
        setTheme(STATE.theme === 'dark' ? 'light' : 'dark');
      });
    }
  }

  function setTheme(theme) {
    STATE.theme = theme;
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('ruview-theme', theme);
    const themeText = document.getElementById('theme-text');
    if (themeText) {
      themeText.textContent = theme === 'dark' ? 'Clair' : 'Sombre';
    }
  }

  function setConnectionStatus(connected, text) {
    STATE.connected = connected;
    const badge = document.getElementById('conn-badge');
    const textEl = document.getElementById('conn-text');
    if (!badge || !textEl) return;

    if (connected) {
      badge.className = 'badge badge-ok';
      textEl.textContent = text || 'Connecté (Live)';
    } else {
      badge.className = 'badge badge-idle';
      textEl.textContent = text || 'Déconnecté';
    }
  }

  function handleFallAlert(active, details) {
    const banner = document.getElementById('alert-banner');
    const detailsEl = document.getElementById('alert-details');
    if (!banner) return;

    if (active) {
      banner.classList.add('active');
      if (detailsEl && details) detailsEl.textContent = details;
    } else {
      banner.classList.remove('active');
    }
  }

  function updatePresence(presenceState, varianceSigma) {
    const valEl = document.getElementById('presence-val');
    const badgeEl = document.getElementById('presence-badge');
    const subEl = document.getElementById('presence-sub');
    if (!valEl) return;

    const stateStr = (presenceState || 'ABSENT').toUpperCase();
    valEl.textContent = stateStr;

    if (stateStr === 'ACTIVE') {
      badgeEl.className = 'badge badge-alert';
      badgeEl.textContent = 'Mouvement Actif';
    } else if (stateStr === 'PASSIVE') {
      badgeEl.className = 'badge badge-warn';
      badgeEl.textContent = 'Respiration Seule';
    } else {
      badgeEl.className = 'badge badge-idle';
      badgeEl.textContent = 'Zone Libre';
    }

    if (subEl) {
      const sig = typeof varianceSigma === 'number' ? varianceSigma.toFixed(2) : '0.00';
      subEl.textContent = `Perturbation: ${sig} σ`;
    }
  }

  function updateBreathing(rateHz, confidence) {
    const bpmEl = document.getElementById('breathing-bpm');
    const subEl = document.getElementById('breathing-sub');
    if (!bpmEl) return;

    if (!rateHz || rateHz <= 0.05) {
      bpmEl.textContent = '--';
      if (subEl) subEl.textContent = 'Fréquence: -- Hz (Conf: 0%)';
      return;
    }

    const bpm = (rateHz * 60).toFixed(1);
    const confPct = Math.round((confidence || 0) * 100);
    bpmEl.textContent = bpm;
    if (subEl) {
      subEl.textContent = `Fréquence: ${rateHz.toFixed(2)} Hz (Conf: ${confPct}%)`;
    }
  }

  function updateMotion(motionVariance, isFall) {
    const valEl = document.getElementById('motion-val');
    const badgeEl = document.getElementById('motion-badge');
    if (!valEl) return;

    const val = typeof motionVariance === 'number' ? motionVariance : 0.0;
    valEl.textContent = val.toFixed(2);

    if (isFall) {
      badgeEl.className = 'badge badge-alert';
      badgeEl.textContent = 'Impact Chute';
    } else if (val > 1.2) {
      badgeEl.className = 'badge badge-warn';
      badgeEl.textContent = 'Agitation';
    } else {
      badgeEl.className = 'badge badge-ok';
      badgeEl.textContent = 'Normal';
    }
  }

  function updateSparkline(value) {
    STATE.sparklineBuffer.push(value);
    if (STATE.sparklineBuffer.length > 100) {
      STATE.sparklineBuffer.shift();
    }

    const min = Math.min(...STATE.sparklineBuffer);
    const max = Math.max(...STATE.sparklineBuffer);
    const range = (max - min) || 1.0;

    const points = STATE.sparklineBuffer.map((v, i) => {
      const x = (i / 99) * 800;
      const normalized = (v - min) / range;
      const y = 70 - (normalized * 60);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });

    const linePath = 'M' + points.join(' L');
    const areaPath = `${linePath} L800,80 L0,80 Z`;

    const lineEl = document.getElementById('spark-line');
    const areaEl = document.getElementById('spark-area');
    if (lineEl) lineEl.setAttribute('d', linePath);
    if (areaEl) areaEl.setAttribute('d', areaPath);
  }

  function renderRfGrid(gridValues) {
    const canvas = document.getElementById('rf-canvas');
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const size = 20;
    const cellW = canvas.width / size;
    const cellH = canvas.height / size;

    ctx.clearRect(0, 0, canvas.width, canvas.height);

    for (let r = 0; r < size; r++) {
      for (let c = 0; c < size; c++) {
        const idx = r * size + c;
        const val = gridValues && gridValues[idx] !== undefined ? gridValues[idx] : 0.0;
        const intensity = Math.min(Math.max(val, 0.0), 1.0);

        // Color ramp from dark blue-gray (0) to cyan (0.5) to amber (1.0)
        let fill;
        if (intensity < 0.5) {
          const t = intensity * 2;
          const rCol = Math.round(15 + t * 20);
          const gCol = Math.round(20 + t * 150);
          const bCol = Math.round(35 + t * 180);
          fill = `rgb(${rCol},${gCol},${bCol})`;
        } else {
          const t = (intensity - 0.5) * 2;
          const rCol = Math.round(35 + t * 210);
          const gCol = Math.round(170 + t * 40);
          const bCol = Math.round(215 - t * 180);
          fill = `rgb(${rCol},${gCol},${bCol})`;
        }

        ctx.fillStyle = fill;
        ctx.fillRect(c * cellW, r * cellH, cellW - 1, cellH - 1);
      }
    }
  }

  function renderPoseSkeleton(keypoints) {
    const canvas = document.getElementById('pose-canvas');
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    if (!keypoints || keypoints.length < 17) {
      ctx.fillStyle = '#64748b';
      ctx.font = '12px system-ui';
      ctx.textAlign = 'center';
      ctx.fillText('Aucune silhouette active', canvas.width / 2, canvas.height / 2);
      return;
    }

    // Draw skeleton bones
    ctx.lineWidth = 2.5;
    ctx.strokeStyle = '#38bdf8';
    for (const [i, j] of SKELETON_PAIRS) {
      const p1 = keypoints[i];
      const p2 = keypoints[j];
      if (!p1 || !p2) continue;
      const conf = Math.min(p1[2] || 0.5, p2[2] || 0.5);
      if (conf < 0.2) continue;

      ctx.globalAlpha = Math.max(0.3, conf);
      ctx.beginPath();
      ctx.moveTo(p1[0] * canvas.width, p1[1] * canvas.height);
      ctx.lineTo(p2[0] * canvas.width, p2[1] * canvas.height);
      ctx.stroke();
    }

    // Draw joints
    ctx.globalAlpha = 1.0;
    for (const p of keypoints) {
      if (!p) continue;
      const conf = p[2] || 0.5;
      if (conf < 0.2) continue;

      ctx.beginPath();
      ctx.arc(p[0] * canvas.width, p[1] * canvas.height, 4, 0, 2 * Math.PI);
      ctx.fillStyle = conf > 0.6 ? '#10b981' : '#f59e0b';
      ctx.fill();
    }
    ctx.globalAlpha = 1.0;
  }

  function updateNodesTable(nodes) {
    const tbody = document.getElementById('nodes-tbody');
    const countEl = document.getElementById('nodes-count');
    const badgeEl = document.getElementById('nodes-badge');
    if (!tbody) return;

    const list = Array.isArray(nodes) ? nodes : [];
    if (countEl) countEl.textContent = list.length;
    if (badgeEl) {
      badgeEl.className = list.length > 0 ? 'badge badge-ok' : 'badge badge-idle';
      badgeEl.textContent = `${list.length} Nœud${list.length > 1 ? 's' : ''}`;
    }

    if (list.length === 0) {
      tbody.innerHTML = `<tr><td colspan="6" style="text-align: center; color: var(--text-muted); padding: 20px;">Aucun nœud ESP32 connecté</td></tr>`;
      return;
    }

    tbody.innerHTML = list.map(n => {
      const rssi = n.rssi ? `${n.rssi} dBm` : '-65 dBm';
      const fps = n.fps ? `${n.fps.toFixed(1)} fps` : '20.0 fps';
      const coh = n.coherence ? (n.coherence * 100).toFixed(0) + '%' : '98%';
      return `<tr>
        <td class="mono" style="font-weight: 600;">${n.id || 'esp32-node'}</td>
        <td class="mono">${n.mac || 'AA:BB:CC:DD:EE:01'}</td>
        <td class="mono">${rssi}</td>
        <td class="mono">${fps}</td>
        <td class="mono">${coh}</td>
        <td><span class="badge badge-ok">Synchro</span></td>
      </tr>`;
    }).join('');
  }

  function dispatchFrame(frame) {
    if (!frame) return;

    if (frame.presence) {
      updatePresence(frame.presence.state, frame.presence.variance);
    }
    if (frame.vital_signs) {
      updateBreathing(frame.vital_signs.breathing_rate_hz, frame.vital_signs.breathing_confidence);
      updateSparkline(frame.vital_signs.breathing_waveform_sample || 40);
    }
    if (frame.fall) {
      const isFall = Boolean(frame.fall.is_fall);
      updateMotion(frame.fall.motion_energy, isFall);
      handleFallAlert(isFall, frame.fall.details);
    }
    if (frame.rf_grid) {
      renderRfGrid(frame.rf_grid);
    }
    if (frame.keypoints) {
      renderPoseSkeleton(frame.keypoints);
    }
    if (frame.nodes) {
      updateNodesTable(frame.nodes);
    }
  }

  function connectWebSocket() {
    clearTimeout(STATE.reconnectTimer);
    const wsProto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsHost = location.host || 'localhost:8080';
    const wsUrl = `${wsProto}//${wsHost}/ws/sensing`;

    try {
      STATE.ws = new WebSocket(wsUrl);

      STATE.ws.onopen = () => {
        setConnectionStatus(true, 'Connecté (Live)');
      };

      STATE.ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          dispatchFrame(data);
        } catch {
          // Ignore malformed packets gracefully
        }
      };

      STATE.ws.onclose = () => {
        setConnectionStatus(false, 'Déconnecté — Reconnexion...');
        STATE.reconnectTimer = setTimeout(connectWebSocket, 3000);
      };

      STATE.ws.onerror = () => {
        STATE.ws.close();
      };
    } catch {
      setConnectionStatus(false, 'Erreur de connexion');
      STATE.reconnectTimer = setTimeout(connectWebSocket, 5000);
    }
  }

  // Dismiss button binding
  document.addEventListener('DOMContentLoaded', () => {
    initTheme();

    const dismissBtn = document.getElementById('dismiss-alert');
    if (dismissBtn) {
      dismissBtn.addEventListener('click', () => {
        handleFallAlert(false);
      });
    }

    renderRfGrid(null);
    renderPoseSkeleton(null);
    connectWebSocket();
  });
})();

