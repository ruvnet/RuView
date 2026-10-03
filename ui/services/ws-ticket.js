// Single-use WebSocket tickets (ADR-272).
//
// A browser's WebSocket constructor cannot set an `Authorization` header on the
// upgrade request. That used to mean the sensing WebSocket stayed reachable
// with no credential even when the server had auth switched on — the REST
// control plane was locked while the live sensing stream was open.
//
// The server now gates `/ws/*` and `/api/v1/stream/pose`. Browsers exchange
// their stored bearer token at `POST /api/v1/ws-ticket` — an ordinary request,
// where headers DO work — for a short-lived, single-use ticket, and pass that
// as `?ticket=` on the socket URL.
//
// Why a token in a URL is acceptable here when it normally is not: the ticket
// is consumed on first use, lives ~30 seconds, and authorizes one WebSocket
// and nothing else. It cannot be replayed against /api/v1/*. The long-lived
// bearer token is still never put in a URL.

import { API_TOKEN_STORAGE_KEY } from './api.service.js';

function storedToken() {
  try {
    return localStorage.getItem(API_TOKEN_STORAGE_KEY) || null;
  } catch {
    // Private browsing / storage disabled — treat as "no token configured".
    return null;
  }
}

/**
 * Mint a ticket. Resolves to `{ ticket, authRequired }`:
 *
 * - `ticket` is null when no token is configured (auth is off, so no ticket is
 *   needed) or when the server does not offer the endpoint.
 * - `authRequired` is `'rejected'` when the stored token was refused (401/403),
 *   so the caller can stop retrying with it. It is null otherwise.
 */
async function mintTicket() {
  const token = storedToken();
  if (!token) return { ticket: null, authRequired: null };

  try {
    const resp = await fetch('/api/v1/ws-ticket', {
      method: 'POST',
      headers: { Authorization: `Bearer ${token}` },
    });
    // 404 means a server predating ADR-272: it still exempts WebSockets, so
    // connecting without a ticket is correct there. Treated as "no ticket
    // needed" rather than as an error, so the UI works against both.
    if (resp.status === 404) return { ticket: null, authRequired: null };
    // #2099: a wrong or revoked token. Retrying with it can never succeed.
    if (resp.status === 401 || resp.status === 403) {
      console.warn('[ws-ticket] stored API token was rejected:', resp.status);
      return { ticket: null, authRequired: 'rejected' };
    }
    if (!resp.ok) {
      console.warn('[ws-ticket] mint failed:', resp.status);
      return { ticket: null, authRequired: null };
    }
    const body = await resp.json();
    return { ticket: body.ticket || null, authRequired: null };
  } catch (err) {
    // Offline, or the server is down. The socket attempt will fail on its own
    // and the caller's reconnect logic handles it; failing loudly here would
    // just duplicate that.
    console.warn('[ws-ticket] mint error:', err.message);
    return { ticket: null, authRequired: null };
  }
}

/**
 * Like `withWsTicket`, but also reports when the stored token was rejected.
 * Resolves to `{ url, authRequired }` with `authRequired` null or `'rejected'`.
 *
 * @param {string} url  ws:// or wss:// URL
 * @returns {Promise<{url: string, authRequired: null|'rejected'}>}
 */
export async function resolveWsUrl(url) {
  const { ticket, authRequired } = await mintTicket();
  if (!ticket) return { url, authRequired };
  const sep = url.includes('?') ? '&' : '?';
  return { url: `${url}${sep}ticket=${encodeURIComponent(ticket)}`, authRequired: null };
}

/**
 * #2099: ask the server whether a socket refusal was an auth refusal.
 *
 * A browser cannot see the status of a refused WebSocket upgrade; it only gets
 * a close event. So after an unticketed socket closes without ever opening,
 * repeat the ticket request with no bearer. The server answers 401 exactly
 * when it requires a credential the browser does not have (a signed-in session
 * cookie still counts, since `fetch` sends it same-origin). Returns
 * `'missing'` in that case and null for anything else, including a network
 * failure, which is an outage rather than an auth problem.
 *
 * Only called on the failure path, so a server with auth off sees no extra
 * request while it is reachable.
 *
 * @returns {Promise<null|'missing'>}
 */
export async function probeWsAuthRequired() {
  if (storedToken()) return null; // mintTicket already judged the stored token
  try {
    const resp = await fetch('/api/v1/ws-ticket', {
      method: 'POST',
      credentials: 'same-origin',
    });
    return resp.status === 401 || resp.status === 403 ? 'missing' : null;
  } catch {
    return null;
  }
}

/**
 * Return `url` with a freshly minted ticket appended, or unchanged when no
 * ticket is available or needed.
 *
 * Always call this immediately before opening the socket — a ticket expires in
 * seconds and is valid exactly once, so one must never be cached or reused
 * across reconnects.
 *
 * @param {string} url  ws:// or wss:// URL
 * @returns {Promise<string>}
 */
export async function withWsTicket(url) {
  return (await resolveWsUrl(url)).url;
}
