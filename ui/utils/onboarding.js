// Onboarding Tour - Interactive first-run walkthrough
// Shows on first visit, can be re-triggered from command palette or help

const STORAGE_KEY = 'ruview-onboarding-done';

export class Onboarding {
  constructor(app) {
    this.app = app;
    this.overlay = null;
    this.currentStep = 0;
    this.steps = [];
    this.active = false;
  }

  init() {
    this.defineSteps();
    document.addEventListener('start-onboarding', () => this.start());

    // Auto-start on first visit
    if (!this.isDone()) {
      // Delay to let the app render first
      setTimeout(() => this.start(), 800);
    }
  }

  defineSteps() {
    this.steps = [
      {
        title: 'Welcome to RuView',
        text: 'WiFi-based human pose estimation that works through walls. Let\'s take a quick tour of the dashboard.',
        target: null, // No highlight, centered
        position: 'center'
      },
      {
        title: 'System Status',
        text: 'Monitor your WiFi sensing hardware and API server status in real time. Green means everything is connected.',
        target: '.live-status-panel',
        position: 'bottom'
      },
      {
        title: 'Live Demo',
        text: 'Switch to the Live Demo tab to see real-time pose detection. Connect an ESP32 sensor or use the built-in simulation.',
        target: '[data-tab="demo"]',
        position: 'bottom'
      },
      {
        title: 'Sensing Visualization',
        text: 'The Sensing tab shows a 3D Gaussian splat visualization of WiFi signal fields, with real-time metrics.',
        target: '[data-tab="sensing"]',
        position: 'bottom'
      },
      {
        title: 'Keyboard Shortcuts',
        text: 'Press ? for shortcuts, Ctrl+K for the command palette, or use number keys 1-8 to switch tabs quickly.',
        target: null,
        position: 'center'
      },
      {
        title: 'You\'re all set!',
        text: 'Explore the dashboard, connect hardware, or start the demo. You can replay this tour anytime from the command palette.',
        target: null,
        position: 'center'
      }
    ];
  }

  isDone() {
    try { return localStorage.getItem(STORAGE_KEY) === 'true'; }
    catch { return false; }
  }

  markDone() {
    try { localStorage.setItem(STORAGE_KEY, 'true'); }
    catch { /* noop */ }
  }

  start() {
    this.currentStep = 0;
    this.active = true;
    this.createOverlay();
    this.showStep();
  }

  createOverlay() {
    // Remove existing if any
    this.removeOverlay();

    this.overlay = document.createElement('div');
    this.overlay.className = 'onboarding-overlay';
    this.overlay.setAttribute('role', 'dialog');
    this.overlay.setAttribute('aria-label', 'Onboarding tour');
    this.overlay.setAttribute('aria-modal', 'true');
    document.body.appendChild(this.overlay);
  }

  showStep() {
    if (this.currentStep >= this.steps.length) {
      this.finish();
      return;
    }

    const step = this.steps[this.currentStep];
    const total = this.steps.length;
    const isFirst = this.currentStep === 0;
    const isLast = this.currentStep === total - 1;

    // Clear highlight
    document.querySelectorAll('.onboarding-highlight').forEach(el => el.classList.remove('onboarding-highlight'));

    // Highlight target
    let targetRect = null;
    if (step.target) {
      const targetEl = document.querySelector(step.target);
      if (targetEl) {
        targetEl.classList.add('onboarding-highlight');
        targetRect = targetEl.getBoundingClientRect();
      }
    }

    this.overlay.innerHTML = `
      <div class="onboarding-backdrop"></div>
      <div class="onboarding-tooltip ${step.position}" ${targetRect ? `style="${this.positionTooltip(targetRect, step.position)}"` : ''}>
        <div class="onboarding-progress">
          ${Array.from({ length: total }, (_, i) =>
            `<span class="onboarding-dot ${i === this.currentStep ? 'active' : i < this.currentStep ? 'done' : ''}"></span>`
          ).join('')}
        </div>
        <h3 class="onboarding-title">${step.title}</h3>
        <p class="onboarding-text">${step.text}</p>
        <div class="onboarding-actions">
          <button class="onboarding-skip">Skip tour</button>
          <div class="onboarding-nav">
            ${!isFirst ? '<button class="onboarding-prev">Back</button>' : ''}
            <button class="onboarding-next">${isLast ? 'Get started' : 'Next'}</button>
          </div>
        </div>
      </div>
    `;

    // Bind events
    this.overlay.querySelector('.onboarding-skip').addEventListener('click', () => this.finish());
    this.overlay.querySelector('.onboarding-next').addEventListener('click', () => {
      this.currentStep++;
      this.showStep();
    });
    const prevBtn = this.overlay.querySelector('.onboarding-prev');
    if (prevBtn) {
      prevBtn.addEventListener('click', () => {
        this.currentStep--;
        this.showStep();
      });
    }
    this.overlay.querySelector('.onboarding-backdrop').addEventListener('click', () => this.finish());

    // Focus next button
    this.overlay.querySelector('.onboarding-next').focus();

    // Escape to close
    this._escHandler = (e) => { if (e.key === 'Escape') this.finish(); };
    document.addEventListener('keydown', this._escHandler);
  }

  positionTooltip(rect, position) {
    const margin = 12;
    if (position === 'bottom') {
      return `left: ${Math.max(16, rect.left + rect.width / 2 - 180)}px; top: ${rect.bottom + margin}px;`;
    }
    if (position === 'top') {
      return `left: ${Math.max(16, rect.left + rect.width / 2 - 180)}px; bottom: ${window.innerHeight - rect.top + margin}px;`;
    }
    return '';
  }

  finish() {
    this.active = false;
    this.markDone();
    this.removeOverlay();
    document.querySelectorAll('.onboarding-highlight').forEach(el => el.classList.remove('onboarding-highlight'));
    if (this._escHandler) document.removeEventListener('keydown', this._escHandler);
  }

  removeOverlay() {
    if (this.overlay?.parentNode) {
      this.overlay.parentNode.removeChild(this.overlay);
      this.overlay = null;
    }
  }

  dispose() {
    this.finish();
  }
}
