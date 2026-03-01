/**
 * Edge-Net Embed Snippet
 *
 * Minimal embed code for websites to include edge-net
 *
 * Usage:
 *   <script src="https://cdn.jsdelivr.net/npm/@ruvector/edge-net/embed.min.js"
 *           data-site-id="your-site-id"
 *           data-cpu-limit="30"
 *           data-show-badge="true">
 *   </script>
 */

(function() {
  'use strict';

  // Get configuration from script tag
  const script = document.currentScript;
  const config = {
    siteId: script.getAttribute('data-site-id') || 'unknown',
    cpuLimit: parseFloat(script.getAttribute('data-cpu-limit') || '30') / 100,
    showBadge: script.getAttribute('data-show-badge') !== 'false',
    badgePosition: script.getAttribute('data-badge-position') || 'bottom-right',
    consentRequired: script.getAttribute('data-consent-required') !== 'false',
    debug: script.getAttribute('data-debug') === 'true',
  };

  // CDN URLs
  const CDN_BASE = 'https://cdn.jsdelivr.net/npm/@ruvector/edge-net@latest';
  const WASM_URL = `${CDN_BASE}/dist/edge-net.wasm`;
  const JS_URL = `${CDN_BASE}/dist/edge-net.min.js`;

  // Logger
  function log(...args) {
    if (config.debug) {
      console.log('[Edge-Net]', ...args);
    }
  }

  // Storage keys
  const CONSENT_KEY = 'edge-net-consent';
  const NODE_KEY = 'edge-net-node';

  // Check consent
  function hasConsent() {
    return localStorage.getItem(CONSENT_KEY) === 'true';
  }

  // Show consent banner
  function showConsentBanner() {
    const banner = document.createElement('div');
    banner.id = 'edge-net-consent-banner';
    banner.innerHTML = `
      <style>
        #edge-net-consent-banner {
          position: fixed;
          bottom: 0;
          left: 0;
          right: 0;
          background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
          color: white;
          padding: 1rem 2rem;
          display: flex;
          align-items: center;
          justify-content: space-between;
          flex-wrap: wrap;
          gap: 1rem;
          z-index: 10000;
          font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
          box-shadow: 0 -4px 20px rgba(0, 0, 0, 0.3);
        }
        #edge-net-consent-banner .content {
          flex: 1;
          min-width: 200px;
        }
        #edge-net-consent-banner h4 {
          margin: 0 0 0.5rem 0;
          font-size: 1rem;
          color: #00d4ff;
        }
        #edge-net-consent-banner p {
          margin: 0;
          font-size: 0.9rem;
          color: #888;
        }
        #edge-net-consent-banner .buttons {
          display: flex;
          gap: 0.75rem;
        }
        #edge-net-consent-banner button {
          padding: 0.6rem 1.2rem;
          border: none;
          border-radius: 6px;
          cursor: pointer;
          font-size: 0.9rem;
          transition: transform 0.2s;
        }
        #edge-net-consent-banner button:hover {
          transform: translateY(-2px);
        }
        #edge-net-consent-banner .accept {
          background: linear-gradient(90deg, #00d4ff, #7b2cbf);
          color: white;
        }
        #edge-net-consent-banner .decline {
          background: rgba(255, 255, 255, 0.1);
          color: white;
          border: 1px solid rgba(255, 255, 255, 0.2);
        }
        #edge-net-consent-banner .learn-more {
          color: #00d4ff;
          text-decoration: underline;
          background: none;
          padding: 0;
        }
      </style>
      <div class="content">
        <h4>Help power AI features</h4>
        <p>Contribute idle compute to earn <strong>rUv</strong> (Resource Utility Vouchers). <button class="learn-more">Learn more</button></p>
      </div>
      <div class="buttons">
        <button class="decline">Not now</button>
        <button class="accept">Accept & Earn rUv</button>
      </div>
    `;

    document.body.appendChild(banner);

    // Event handlers
    banner.querySelector('.accept').addEventListener('click', () => {
      localStorage.setItem(CONSENT_KEY, 'true');
      banner.remove();
      init();
    });

    banner.querySelector('.decline').addEventListener('click', () => {
      localStorage.setItem(CONSENT_KEY, 'false');
      banner.remove();
    });

    banner.querySelector('.learn-more').addEventListener('click', () => {
      window.open('https://github.com/ruvnet/ruvector/tree/main/examples/edge-net', '_blank');
    });
  }

  // Create badge element
  function createBadge() {
    const badge = document.createElement('div');
    badge.id = 'edge-net-badge';

    const positions = {
      'bottom-right': 'bottom: 20px; right: 20px;',
      'bottom-left': 'bottom: 20px; left: 20px;',
      'top-right': 'top: 20px; right: 20px;',
      'top-left': 'top: 20px; left: 20px;',
    };

    badge.innerHTML = `
      <style>
        #edge-net-badge {
          position: fixed;
          ${positions[config.badgePosition]}
          background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
          color: white;
          padding: 0.75rem 1rem;
          border-radius: 12px;
          font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
          font-size: 0.85rem;
          z-index: 9999;
          box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
          border: 1px solid rgba(255, 255, 255, 0.1);
          cursor: pointer;
          transition: all 0.3s;
          display: flex;
          align-items: center;
          gap: 0.75rem;
        }
        #edge-net-badge:hover {
          transform: translateY(-2px);
          box-shadow: 0 6px 24px rgba(0, 212, 255, 0.2);
        }
        #edge-net-badge.minimized {
          padding: 0.5rem;
          border-radius: 50%;
        }
        #edge-net-badge.minimized .details {
          display: none;
        }
        #edge-net-badge .status {
          width: 10px;
          height: 10px;
          border-radius: 50%;
          background: #00ff88;
          animation: edge-net-pulse 2s infinite;
        }
        #edge-net-badge .status.paused {
          background: #ffaa00;
          animation: none;
        }
        #edge-net-badge .status.error {
          background: #ff6b6b;
          animation: none;
        }
        #edge-net-badge .balance {
          color: #00ff88;
          font-weight: bold;
        }
        #edge-net-badge .multiplier {
          color: #ff6b6b;
          font-size: 0.75rem;
        }
        @keyframes edge-net-pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.5; }
        }
      </style>
      <div class="status"></div>
      <div class="details">
        <span class="balance">0 rUv</span>
        <span class="multiplier">• 10.0x</span>
      </div>
    `;

    document.body.appendChild(badge);

    // Toggle minimize on click
    badge.addEventListener('click', () => {
      badge.classList.toggle('minimized');
    });

    return badge;
  }

  // Update badge
  function updateBadge(badge, stats) {
    const balanceEl = badge.querySelector('.balance');
    const multiplierEl = badge.querySelector('.multiplier');
    const statusEl = badge.querySelector('.status');

    if (balanceEl) balanceEl.textContent = `${stats.balance.toFixed(2)} rUv`;
    if (multiplierEl) multiplierEl.textContent = `• ${stats.multiplier.toFixed(1)}x`;

    if (statusEl) {
      statusEl.classList.remove('paused', 'error');
      if (stats.paused) statusEl.classList.add('paused');
      if (stats.error) statusEl.classList.add('error');
    }
  }

  // Load Edge-Net module
  async function loadModule() {
    log('Loading Edge-Net module...');

    // Dynamic import from CDN
    const module = await import(JS_URL);
    return module.EdgeNet;
  }

  // Initialize Edge-Net
  async function init() {
    try {
      log('Initializing with config:', config);

      const EdgeNet = await loadModule();

      const node = await EdgeNet.init({
        siteId: config.siteId,
        contribution: config.cpuLimit,
        wasmUrl: WASM_URL,
        onCredit: (earned, total) => {
          log(`Earned ${earned} QDAG, total: ${total}`);
        },
        onError: (error) => {
          console.error('[Edge-Net] Error:', error);
        },
      });

      // Create badge if enabled
      let badge = null;
      if (config.showBadge) {
        badge = createBadge();
      }

      // Update loop
      setInterval(() => {
        const stats = node.getStats();
        if (badge) {
          updateBadge(badge, stats);
        }
      }, 1000);

      // Expose to window for debugging
      window.EdgeNetNode = node;

      log('Edge-Net initialized successfully');

      // Dispatch ready event
      window.dispatchEvent(new CustomEvent('edge-net-ready', { detail: { node } }));

    } catch (error) {
      console.error('[Edge-Net] Failed to initialize:', error);
    }
  }

  // Entry point
  function main() {
    // Wait for DOM
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', main);
      return;
    }

    log('Edge-Net embed script loaded');

    // Check consent
    if (config.consentRequired && !hasConsent()) {
      showConsentBanner();
    } else if (hasConsent() || !config.consentRequired) {
      init();
    }
  }

  main();
})();
