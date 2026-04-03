// Mobile Navigation - Hamburger menu for small screens
// Replaces wrapped tab bar with a slide-out drawer on mobile

export class MobileNav {
  constructor() {
    this.drawer = null;
    this.backdrop = null;
    this.hamburger = null;
    this.isOpen = false;
    this.mql = window.matchMedia('(max-width: 768px)');
  }

  init() {
    this.createHamburger();
    this.createDrawer();
    this.bindEvents();
    this.onMediaChange(this.mql);
  }

  createHamburger() {
    this.hamburger = document.createElement('button');
    this.hamburger.className = 'mobile-hamburger';
    this.hamburger.setAttribute('aria-label', 'Open navigation menu');
    this.hamburger.setAttribute('aria-expanded', 'false');
    this.hamburger.innerHTML = `
      <span class="hamburger-line"></span>
      <span class="hamburger-line"></span>
      <span class="hamburger-line"></span>
    `;
    this.hamburger.addEventListener('click', () => this.toggle());

    const header = document.querySelector('.header');
    if (header) {
      header.style.position = 'relative';
      header.appendChild(this.hamburger);
    }
  }

  createDrawer() {
    // Backdrop
    this.backdrop = document.createElement('div');
    this.backdrop.className = 'mobile-nav-backdrop';
    this.backdrop.addEventListener('click', () => this.close());
    document.body.appendChild(this.backdrop);

    // Drawer
    this.drawer = document.createElement('nav');
    this.drawer.className = 'mobile-nav-drawer';
    this.drawer.setAttribute('role', 'navigation');
    this.drawer.setAttribute('aria-label', 'Mobile navigation');

    // Clone tabs into drawer
    const tabs = document.querySelectorAll('.nav-tabs .nav-tab');
    const list = document.createElement('div');
    list.className = 'mobile-nav-list';

    tabs.forEach(tab => {
      const item = document.createElement(tab.tagName === 'A' ? 'a' : 'button');
      item.className = 'mobile-nav-item';
      item.textContent = tab.textContent.trim();

      if (tab.tagName === 'A') {
        item.href = tab.href;
      } else {
        const tabId = tab.getAttribute('data-tab');
        item.dataset.tab = tabId;
        if (tab.classList.contains('active')) {
          item.classList.add('active');
        }
        item.addEventListener('click', () => {
          // Activate tab via the original tab manager
          tab.click();
          this.close();
          // Update active states in drawer
          list.querySelectorAll('.mobile-nav-item').forEach(i => i.classList.remove('active'));
          item.classList.add('active');
        });
      }

      list.appendChild(item);
    });

    this.drawer.appendChild(list);

    // Keyboard hint at bottom
    const hint = document.createElement('div');
    hint.className = 'mobile-nav-hint';
    hint.textContent = 'Tip: Press Ctrl+K for command palette';
    this.drawer.appendChild(hint);

    document.body.appendChild(this.drawer);

    // Sync active tab when tabs change externally
    const observer = new MutationObserver(() => {
      const activeTab = document.querySelector('.nav-tabs .nav-tab.active');
      if (activeTab) {
        const activeId = activeTab.getAttribute('data-tab');
        list.querySelectorAll('.mobile-nav-item').forEach(item => {
          item.classList.toggle('active', item.dataset.tab === activeId);
        });
      }
    });

    const navTabs = document.querySelector('.nav-tabs');
    if (navTabs) {
      observer.observe(navTabs, { attributes: true, subtree: true, attributeFilter: ['class'] });
    }
  }

  bindEvents() {
    // Listen for media query changes
    this.mql.addEventListener('change', (e) => this.onMediaChange(e));

    // Close on escape
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && this.isOpen) this.close();
    });

    // Swipe to close
    let touchStartX = 0;
    this.drawer.addEventListener('touchstart', (e) => {
      touchStartX = e.touches[0].clientX;
    }, { passive: true });
    this.drawer.addEventListener('touchend', (e) => {
      const deltaX = e.changedTouches[0].clientX - touchStartX;
      if (deltaX < -50) this.close(); // Swipe left to close
    }, { passive: true });
  }

  onMediaChange(mql) {
    const isMobile = mql.matches !== undefined ? mql.matches : mql;
    document.body.classList.toggle('mobile-nav-active', isMobile);

    if (!isMobile && this.isOpen) {
      this.close();
    }
  }

  toggle() {
    this.isOpen ? this.close() : this.open();
  }

  open() {
    this.isOpen = true;
    this.drawer.classList.add('open');
    this.backdrop.classList.add('open');
    this.hamburger.classList.add('open');
    this.hamburger.setAttribute('aria-expanded', 'true');
    document.body.style.overflow = 'hidden';

    // Focus first item
    const first = this.drawer.querySelector('.mobile-nav-item');
    if (first) first.focus();
  }

  close() {
    this.isOpen = false;
    this.drawer.classList.remove('open');
    this.backdrop.classList.remove('open');
    this.hamburger.classList.remove('open');
    this.hamburger.setAttribute('aria-expanded', 'false');
    document.body.style.overflow = '';
  }

  dispose() {
    this.close();
    this.hamburger?.remove();
    this.drawer?.remove();
    this.backdrop?.remove();
  }
}
