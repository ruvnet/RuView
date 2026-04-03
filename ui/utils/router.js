// Hash Router - Makes tabs bookmarkable and shareable
// URL format: #dashboard, #demo, #sensing, etc.

export class Router {
  constructor(app) {
    this.app = app;
    this.validTabs = ['dashboard', 'hardware', 'demo', 'architecture', 'performance', 'applications', 'sensing', 'training'];
  }

  init() {
    // Navigate to hash on load
    this.onHashChange();

    // Listen for hash changes (back/forward navigation)
    window.addEventListener('hashchange', () => this.onHashChange());

    // Update hash when tab changes
    const tabManager = this.app?.getComponent?.('tabManager');
    if (tabManager) {
      tabManager.onTabChange((tabId) => {
        this.setHash(tabId);
      });
    }
  }

  onHashChange() {
    const hash = window.location.hash.replace('#', '').toLowerCase();
    if (hash && this.validTabs.includes(hash)) {
      const tabManager = this.app?.getComponent?.('tabManager');
      if (tabManager && tabManager.getActiveTab() !== hash) {
        tabManager.switchToTab(hash);
      }
    }
  }

  setHash(tabId) {
    // Only update if different to avoid infinite loop
    const current = window.location.hash.replace('#', '');
    if (current !== tabId) {
      history.replaceState(null, '', `#${tabId}`);
    }
  }

  dispose() {
    // No explicit cleanup needed - event listeners are on window
  }
}
