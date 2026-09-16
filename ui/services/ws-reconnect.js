// Add manual-disconnect suppression to the shared reconnect state machine.
//
// Reviewer objection on the live-view PR: "explicit disconnect can immediately
// schedule reconnection again". The cause: the page's disconnect() closed the
// socket, closing fired onclose, and onclose called scheduleReconnect
// unconditionally. Suppression is a property of the shared state so both page
// controllers get it from one place.

export const RECONNECT_BASE_MS = 1000;
export const RECONNECT_MAX_MS = 15000;
export const RECONNECT_MAX_FAILURES = 4;

/** Shared reconnect bookkeeping, mixed into both page controllers. */
export function reconnectState() {
  return {
    _reconnectTimer: null,
    _reconnectDelay: RECONNECT_BASE_MS,
    _reconnectFailures: 0,
    // True after a deliberate disconnect: closing the socket fires onclose, and
    // without this flag that handler would schedule a retry for a link the operator
    // asked to close.
    _reconnectSuppressed: false,
  };
}

/** Is reconnection suppressed by a manual disconnect? */
export function isReconnectSuppressed(state) {
  return state._reconnectSuppressed === true;
}

/** A deliberate disconnect: cancel anything pending and refuse further retries. */
export function suppressReconnect(state) {
  cancelReconnect(state);
  state._reconnectSuppressed = true;
}

/** An explicit connect: retries are allowed again. */
export function allowReconnect(state) {
  state._reconnectSuppressed = false;
}

/** Schedule one reconnect attempt, with exponential backoff. */
export function scheduleReconnect(state, attempt, onGiveUp) {
  if (state._reconnectSuppressed || state._reconnectTimer) return;
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
