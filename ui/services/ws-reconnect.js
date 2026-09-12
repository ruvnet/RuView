// Reconnect instead of dropping to demo.
//
// A server restart used to leave an open page in the watermarked demo view until
// the operator reloaded it: both pages connected exactly once and treated
// `onclose` as "no live data". Sensor restarts, `cargo run` cycles and Wi-Fi
// hiccups are normal during a deployment, so both pages now retry with backoff
// and only fall back to demo once the link has been down long enough that the
// last verified frame is no longer a fair representation.
//
// The fallback after MAX_FAILURES keeps the ADR-295 watermark honest: a page that
// has been disconnected for tens of seconds must not keep showing a live frame as
// if it were current.

export const RECONNECT_BASE_MS = 1000;
export const RECONNECT_MAX_MS = 15000;
export const RECONNECT_MAX_FAILURES = 4;

/** Shared reconnect bookkeeping, mixed into both page controllers. */
export function reconnectState() {
  return {
    _reconnectTimer: null,
    _reconnectDelay: RECONNECT_BASE_MS,
    _reconnectFailures: 0,
  };
}

/** Schedule one reconnect attempt, with exponential backoff. */
export function scheduleReconnect(state, attempt, onGiveUp) {
  if (state._reconnectTimer) return;
  const delay = state._reconnectDelay;
  state._reconnectTimer = setTimeout(() => {
    state._reconnectTimer = null;
    state._reconnectDelay = Math.min(state._reconnectDelay * 2, RECONNECT_MAX_MS);
    state._reconnectFailures += 1;
    if (state._reconnectFailures > RECONNECT_MAX_FAILURES) {
      onGiveUp();
      return;
    }
    attempt();
  }, delay);
}

/** Cancel a pending reconnect (manual disconnect, or a link that came back). */
export function cancelReconnect(state) {
  if (state._reconnectTimer) {
    clearTimeout(state._reconnectTimer);
    state._reconnectTimer = null;
  }
}

/** Reset the backoff after a verified frame. */
export function reconnectSucceeded(state) {
  cancelReconnect(state);
  state._reconnectDelay = RECONNECT_BASE_MS;
  state._reconnectFailures = 0;
}
