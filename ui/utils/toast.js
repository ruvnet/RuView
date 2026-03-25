// Enhanced Toast Notification System
// Supports multiple types: success, error, warning, info
// Stacking, auto-dismiss, manual close, progress bar

export class ToastManager {
  constructor() {
    this.container = null;
    this.toasts = [];
    this.idCounter = 0;
  }

  init() {
    this.container = document.createElement('div');
    this.container.className = 'toast-container';
    this.container.setAttribute('role', 'region');
    this.container.setAttribute('aria-label', 'Notifications');
    this.container.setAttribute('aria-live', 'polite');
    document.body.appendChild(this.container);
  }

  show(message, options = {}) {
    const {
      type = 'info',
      duration = 5000,
      closable = true,
      icon = null,
      action = null
    } = options;

    const id = ++this.idCounter;
    const toast = document.createElement('div');
    toast.className = `toast toast-${type}`;
    toast.setAttribute('role', 'alert');
    toast.dataset.toastId = id;

    const iconMap = {
      success: '<svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M13.5 4.5L6 12L2.5 8.5" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>',
      error: '<svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M12 4L4 12M4 4l8 8" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>',
      warning: '<svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M8 5v4M8 11h.01" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M7.13 2.22L1.09 12.5a1 1 0 00.87 1.5h12.08a1 1 0 00.87-1.5L8.87 2.22a1 1 0 00-1.74 0z" stroke="currentColor" stroke-width="1.5"/></svg>',
      info: '<svg width="16" height="16" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="8" r="6.5" stroke="currentColor" stroke-width="1.5"/><path d="M8 7v4M8 5h.01" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>'
    };

    const displayIcon = icon || iconMap[type] || iconMap.info;

    toast.innerHTML = `
      <div class="toast-icon">${displayIcon}</div>
      <div class="toast-content">
        <span class="toast-message">${this.escapeHtml(message)}</span>
        ${action ? `<button class="toast-action">${this.escapeHtml(action.label)}</button>` : ''}
      </div>
      ${closable ? '<button class="toast-dismiss" aria-label="Dismiss">&times;</button>' : ''}
      ${duration > 0 ? '<div class="toast-progress"><div class="toast-progress-bar"></div></div>' : ''}
    `;

    // Bind events
    if (closable) {
      toast.querySelector('.toast-dismiss').addEventListener('click', () => this.dismiss(id));
    }
    if (action?.onClick) {
      toast.querySelector('.toast-action')?.addEventListener('click', () => {
        action.onClick();
        this.dismiss(id);
      });
    }

    this.container.appendChild(toast);

    // Trigger enter animation
    requestAnimationFrame(() => toast.classList.add('toast-enter'));

    // Auto-dismiss
    let timeoutId = null;
    if (duration > 0) {
      const progressBar = toast.querySelector('.toast-progress-bar');
      if (progressBar) {
        progressBar.style.animationDuration = `${duration}ms`;
        progressBar.classList.add('toast-progress-animate');
      }
      timeoutId = setTimeout(() => this.dismiss(id), duration);
    }

    // Pause on hover
    toast.addEventListener('mouseenter', () => {
      if (timeoutId) {
        clearTimeout(timeoutId);
        const bar = toast.querySelector('.toast-progress-bar');
        if (bar) bar.style.animationPlayState = 'paused';
      }
    });
    toast.addEventListener('mouseleave', () => {
      if (duration > 0) {
        const bar = toast.querySelector('.toast-progress-bar');
        if (bar) bar.style.animationPlayState = 'running';
        timeoutId = setTimeout(() => this.dismiss(id), duration / 2);
      }
    });

    this.toasts.push({ id, toast, timeoutId });
    return id;
  }

  dismiss(id) {
    const index = this.toasts.findIndex(t => t.id === id);
    if (index === -1) return;

    const { toast, timeoutId } = this.toasts[index];
    if (timeoutId) clearTimeout(timeoutId);

    toast.classList.add('toast-exit');
    toast.addEventListener('animationend', () => {
      toast.remove();
    }, { once: true });

    this.toasts.splice(index, 1);
  }

  success(message, options = {}) {
    return this.show(message, { ...options, type: 'success' });
  }

  error(message, options = {}) {
    return this.show(message, { ...options, type: 'error', duration: options.duration || 8000 });
  }

  warning(message, options = {}) {
    return this.show(message, { ...options, type: 'warning', duration: options.duration || 6000 });
  }

  info(message, options = {}) {
    return this.show(message, { ...options, type: 'info' });
  }

  escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  dispose() {
    this.toasts.forEach(({ timeoutId }) => {
      if (timeoutId) clearTimeout(timeoutId);
    });
    this.toasts = [];
    if (this.container?.parentNode) {
      this.container.parentNode.removeChild(this.container);
    }
  }
}

export const toastManager = new ToastManager();
