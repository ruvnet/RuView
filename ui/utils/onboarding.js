// Onboarding Tour - Interactive first-run walkthrough
// Shows on first visit, can be re-triggered from command palette or help
import { i18n } from './i18n.js';

const STORAGE_KEY = 'ruview-onboarding-done';

export class Onboarding {
  constructor(app) {
    this.app = app;
    this.overlay = null;
    this.currentStep = 0;
    this.steps = [];
    this.active = false;
    this.selectedLocale = i18n.locale;
  }

  init() {
    this.defineSteps();
    this._localeUnsub = i18n.onLocaleChange(() => {
      if (this.active) this.showStep();
    });
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
        titleKey: 'onboarding.step1.title',
        textKey: 'onboarding.step1.text',
        target: null, // No highlight, centered
        position: 'center'
      },
      {
        titleKey: 'onboarding.step2.title',
        textKey: 'onboarding.step2.text',
        target: '.live-status-panel',
        position: 'bottom'
      },
      {
        titleKey: 'onboarding.step3.title',
        textKey: 'onboarding.step3.text',
        target: '[data-tab="demo"]',
        position: 'bottom'
      },
      {
        titleKey: 'onboarding.step4.title',
        textKey: 'onboarding.step4.text',
        target: '[data-tab="sensing"]',
        position: 'bottom'
      },
      {
        titleKey: 'onboarding.step5.title',
        textKey: 'onboarding.step5.text',
        target: null,
        position: 'center'
      },
      {
        titleKey: 'onboarding.step6.title',
        textKey: 'onboarding.step6.text',
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
    this.currentStep = -1;
    this.active = true;
    this.selectedLocale = i18n.locale;
    this.createOverlay();
    this.showStep();
  }

  createOverlay() {
    // Remove existing if any
    this.removeOverlay();

    this.overlay = document.createElement('div');
    this.overlay.className = 'onboarding-overlay';
    this.overlay.setAttribute('role', 'dialog');
    this.overlay.setAttribute('aria-label', i18n.t('onboarding.ariaLabel'));
    this.overlay.setAttribute('aria-modal', 'true');
    document.body.appendChild(this.overlay);
  }

  showStep() {
    if (this._escHandler) document.removeEventListener('keydown', this._escHandler);
    if (this.currentStep < 0) {
      this.showLanguageStep();
      return;
    }

    if (this.currentStep >= this.steps.length) {
      this.finish();
      return;
    }

    const step = this.steps[this.currentStep];
    const total = this.steps.length;
    const isFirst = this.currentStep === 0;
    const isLast = this.currentStep === total - 1;
    const title = i18n.t(step.titleKey);
    const text = i18n.t(step.textKey);
    const skipLabel = i18n.t('onboarding.skip');
    const backLabel = i18n.t('onboarding.back');
    const nextLabel = i18n.t('onboarding.next');
    const getStartedLabel = i18n.t('onboarding.getStarted');

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
        <h3 class="onboarding-title">${title}</h3>
        <p class="onboarding-text">${text}</p>
        <div class="onboarding-actions">
          <button class="onboarding-skip">${skipLabel}</button>
          <div class="onboarding-nav">
            ${!isFirst ? `<button class="onboarding-prev">${backLabel}</button>` : ''}
            <button class="onboarding-next">${isLast ? getStartedLabel : nextLabel}</button>
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

  showLanguageStep() {
    if (this._escHandler) document.removeEventListener('keydown', this._escHandler);
    document.querySelectorAll('.onboarding-highlight').forEach(el => el.classList.remove('onboarding-highlight'));

    const title = i18n.t('onboarding.languageTitle');
    const text = i18n.t('onboarding.languageText');
    const skipLabel = i18n.t('onboarding.skip');
    const continueLabel = i18n.t('onboarding.continue');
    const locale = this.selectedLocale || i18n.locale;

    this.overlay.innerHTML = `
      <div class="onboarding-backdrop"></div>
      <div class="onboarding-tooltip center onboarding-tooltip--language">
        <div class="onboarding-progress">
          <span class="onboarding-dot active"></span>
        </div>
        <h3 class="onboarding-title">${title}</h3>
        <p class="onboarding-text">${text}</p>
        <div class="onboarding-language-grid">
          <button class="onboarding-language-btn ${locale === 'pt-BR' ? 'active' : ''}" data-locale="pt-BR">Português (Brasil)</button>
          <button class="onboarding-language-btn ${locale === 'en' ? 'active' : ''}" data-locale="en">English</button>
          <button class="onboarding-language-btn ${locale === 'pl' ? 'active' : ''}" data-locale="pl">Polski</button>
        </div>
        <div class="onboarding-actions">
          <button class="onboarding-skip">${skipLabel}</button>
          <div class="onboarding-nav">
            <button class="onboarding-next">${continueLabel}</button>
          </div>
        </div>
      </div>
    `;

    this.overlay.querySelectorAll('.onboarding-language-btn').forEach(button => {
      button.addEventListener('click', () => {
        this.selectedLocale = button.dataset.locale;
        i18n.setLocale(this.selectedLocale);
        this.showLanguageStep();
      });
    });

    this.overlay.querySelector('.onboarding-skip').addEventListener('click', () => this.finish());
    this.overlay.querySelector('.onboarding-next').addEventListener('click', () => {
      if (this.selectedLocale) i18n.setLocale(this.selectedLocale);
      this.currentStep = 0;
      this.showStep();
    });
    this.overlay.querySelector('.onboarding-backdrop').addEventListener('click', () => this.finish());
    this.overlay.querySelector('.onboarding-next').focus();

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
    if (this._localeUnsub) this._localeUnsub();
    this.finish();
  }
}
