// Idle Manager - Pauses animations, polling, and WebSocket pings when user is inactive
// Reduces CPU/battery usage on idle dashboards

export class IdleManager {
  constructor() {
    this.idleTimeout = 3 * 60 * 1000; // 3 minutes
    this.isIdle = false;
    this.timer = null;
    this.callbacks = { idle: [], active: [] };
    this.events = ['mousemove', 'mousedown', 'keydown', 'touchstart', 'scroll'];
  }

  init() {
    this.resetTimer();
    this.events.forEach(evt => {
      document.addEventListener(evt, () => this.onActivity(), { passive: true, capture: true });
    });
    // Also use Page Visibility API
    document.addEventListener('visibilitychange', () => {
      if (document.hidden) {
        this.goIdle();
      } else {
        this.goActive();
      }
    });
  }

  onActivity() {
    if (this.isIdle) {
      this.goActive();
    }
    this.resetTimer();
  }

  resetTimer() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => this.goIdle(), this.idleTimeout);
  }

  goIdle() {
    if (this.isIdle) return;
    this.isIdle = true;
    console.info('[Idle] User inactive - pausing background tasks');
    this.notify('idle');
    document.body.classList.add('user-idle');
  }

  goActive() {
    if (!this.isIdle) return;
    this.isIdle = false;
    console.info('[Idle] User active - resuming background tasks');
    this.notify('active');
    document.body.classList.remove('user-idle');
    this.resetTimer();
  }

  onIdle(callback) {
    this.callbacks.idle.push(callback);
    return () => {
      const i = this.callbacks.idle.indexOf(callback);
      if (i > -1) this.callbacks.idle.splice(i, 1);
    };
  }

  onActive(callback) {
    this.callbacks.active.push(callback);
    return () => {
      const i = this.callbacks.active.indexOf(callback);
      if (i > -1) this.callbacks.active.splice(i, 1);
    };
  }

  notify(type) {
    this.callbacks[type].forEach(cb => {
      try { cb(); } catch (e) { console.error('[Idle] Callback error:', e); }
    });
  }

  dispose() {
    if (this.timer) clearTimeout(this.timer);
    this.callbacks = { idle: [], active: [] };
  }
}
