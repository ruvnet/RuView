// Fullscreen Mode - Toggle fullscreen on visualization tabs
// Activated via F11 key, command palette, or button

export class FullscreenManager {
  constructor() {
    this.isFullscreen = false;
    this.targetElement = null;
  }

  init() {
    document.addEventListener('toggle-fullscreen', () => this.toggle());

    document.addEventListener('keydown', (e) => {
      if (e.key === 'F11') {
        e.preventDefault();
        this.toggle();
      }
    });

    document.addEventListener('fullscreenchange', () => {
      this.isFullscreen = !!document.fullscreenElement;
      this.updateUI();
    });
  }

  toggle() {
    if (this.isFullscreen) {
      this.exit();
    } else {
      this.enter();
    }
  }

  enter() {
    // Find the active tab content
    const activePanel = document.querySelector('.tab-content.active');
    if (!activePanel) return;

    this.targetElement = activePanel;

    if (activePanel.requestFullscreen) {
      activePanel.requestFullscreen();
    } else if (activePanel.webkitRequestFullscreen) {
      activePanel.webkitRequestFullscreen();
    }
  }

  exit() {
    if (document.exitFullscreen) {
      document.exitFullscreen();
    } else if (document.webkitExitFullscreen) {
      document.webkitExitFullscreen();
    }
    this.targetElement = null;
  }

  updateUI() {
    document.body.classList.toggle('is-fullscreen', this.isFullscreen);

    // Add/remove exit button when in fullscreen
    let exitBtn = document.getElementById('fullscreen-exit-btn');
    if (this.isFullscreen && !exitBtn) {
      exitBtn = document.createElement('button');
      exitBtn.id = 'fullscreen-exit-btn';
      exitBtn.className = 'fullscreen-exit-btn';
      exitBtn.setAttribute('aria-label', 'Exit fullscreen');
      exitBtn.innerHTML = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 14 10 14 10 20"/><polyline points="20 10 14 10 14 4"/><line x1="14" y1="10" x2="21" y2="3"/><line x1="3" y1="21" x2="10" y2="14"/></svg>';
      exitBtn.title = 'Exit fullscreen (F11)';
      exitBtn.addEventListener('click', () => this.exit());
      document.body.appendChild(exitBtn);
    } else if (!this.isFullscreen && exitBtn) {
      exitBtn.remove();
    }
  }

  dispose() {
    if (this.isFullscreen) this.exit();
  }
}
