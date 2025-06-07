// Tab Manager Component

export class TabManager {
  constructor(containerElement) {
    this.container = containerElement;
    this.tabs = [];
    this.activeTab = null;
    this.tabChangeCallbacks = [];
  }

  // Initialize tabs
  init() {
    // Find all tabs and contents
    this.tabs = Array.from(this.container.querySelectorAll('.nav-tab'));
    this.tabContents = Array.from(this.container.querySelectorAll('.tab-content'));
    
    // Set up event listeners
    this.tabs.forEach(tab => {
      tab.addEventListener('click', () => this.switchTab(tab));
    });

    // Activate first tab if none active
    const activeTab = this.tabs.find(tab => tab.classList.contains('active'));
    if (activeTab) {
      this.activeTab = activeTab.getAttribute('data-tab');
    } else if (this.tabs.length > 0) {
      this.switchTab(this.tabs[0]);
    }
  }

  // Switch to a tab
  switchTab(tabElement) {
    const tabId = tabElement.getAttribute('data-tab');
    
    if (tabId === this.activeTab) {
      return;
    }

    // Update tab states
    this.tabs.forEach(tab => {
      tab.classList.toggle('active', tab === tabElement);
    });

    // Update content visibility
    this.tabContents.forEach(content => {
      content.classList.toggle('active', content.id === tabId);
    });

    // Update active tab
    const previousTab = this.activeTab;
    this.activeTab = tabId;

    // Notify callbacks
    this.notifyTabChange(tabId, previousTab);
  }

  // Switch to tab by ID
  switchToTab(tabId) {
    const tab = this.tabs.find(t => t.getAttribute('data-tab') === tabId);
    if (tab) {
      this.switchTab(tab);
    }
  }

  // Register tab change callback
  onTabChange(callback) {
    this.tabChangeCallbacks.push(callback);
    
    // Return unsubscribe function
    return () => {
      const index = this.tabChangeCallbacks.indexOf(callback);
      if (index > -1) {
        this.tabChangeCallbacks.splice(index, 1);
      }
    };
  }

  // Notify tab change callbacks
  notifyTabChange(newTab, previousTab) {
    this.tabChangeCallbacks.forEach(callback => {
      try {
        callback(newTab, previousTab);
      } catch (error) {
        console.error('Error in tab change callback:', error);
      }
    });
  }

  // Get active tab
  getActiveTab() {
    return this.activeTab;
  }

  // Enable/disable tab
  setTabEnabled(tabId, enabled) {
    const tab = this.tabs.find(t => t.getAttribute('data-tab') === tabId);
    if (tab) {
      tab.disabled = !enabled;
      tab.classList.toggle('disabled', !enabled);
    }
  }

  // Show/hide tab
  setTabVisible(tabId, visible) {
    const tab = this.tabs.find(t => t.getAttribute('data-tab') === tabId);
    if (tab) {
      tab.style.display = visible ? '' : 'none';
    }
  }

  // Add badge to tab
  setTabBadge(tabId, badge) {
    const tab = this.tabs.find(t => t.getAttribute('data-tab') === tabId);
    if (!tab) return;

    // Remove existing badge
    const existingBadge = tab.querySelector('.tab-badge');
    if (existingBadge) {
      existingBadge.remove();
    }

    // Add new badge if provided
    if (badge) {
      const badgeElement = document.createElement('span');
      badgeElement.className = 'tab-badge';
      badgeElement.textContent = badge;
      tab.appendChild(badgeElement);
    }
  }

  // Clean up
  dispose() {
    this.tabs.forEach(tab => {
      tab.removeEventListener('click', this.switchTab);
    });
    this.tabChangeCallbacks = [];
  }
}