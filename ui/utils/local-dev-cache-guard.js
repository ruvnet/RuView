const LOOPBACK_HOSTS = new Set(['localhost', '127.0.0.1', '::1']);
const CANONICAL_LOOPBACK_HOST = '127.0.0.1';
const RELOAD_MARKER = '__ruviewCacheReset';

function getCanonicalLoopbackHost() {
  const metaHost = document
    .querySelector('meta[name="ruview-canonical-loopback"]')
    ?.getAttribute('content')
    ?.trim();

  if (metaHost && LOOPBACK_HOSTS.has(metaHost)) {
    return metaHost;
  }

  return CANONICAL_LOOPBACK_HOST;
}

function isLoopbackHostname(hostname = window.location.hostname) {
  return LOOPBACK_HOSTS.has(hostname);
}

export function shouldBypassPersistentCaching(hostname = window.location.hostname) {
  return isLoopbackHostname(hostname);
}

export function redirectToCanonicalLoopback() {
  const { protocol, hostname, port, pathname, search, hash } = window.location;
  const canonicalHost = getCanonicalLoopbackHost();
  if (!isLoopbackHostname(hostname) || hostname === canonicalHost) {
    return false;
  }

  const canonicalUrl = `${protocol}//${canonicalHost}${port ? `:${port}` : ''}${pathname}${search}${hash}`;
  window.location.replace(canonicalUrl);
  return true;
}

export async function resetLocalDevCachesIfNeeded() {
  if (!shouldBypassPersistentCaching()) {
    return false;
  }

  let changed = false;

  if ('serviceWorker' in navigator) {
    const registrations = await navigator.serviceWorker.getRegistrations();
    await Promise.all(
      registrations.map(async (registration) => {
        const unregistered = await registration.unregister();
        changed = changed || unregistered;
      })
    );
  }

  if ('caches' in window) {
    const cacheKeys = await caches.keys();
    if (cacheKeys.length > 0) {
      await Promise.all(cacheKeys.map((key) => caches.delete(key)));
      changed = true;
    }
  }

  return changed;
}

export async function enforceLocalDevFreshness() {
  if (redirectToCanonicalLoopback()) {
    return;
  }

  if (!shouldBypassPersistentCaching()) {
    return;
  }

  const params = new URLSearchParams(window.location.search);
  const alreadyReloaded = params.get(RELOAD_MARKER) === '1';
  const changed = await resetLocalDevCachesIfNeeded();

  if (changed && !alreadyReloaded) {
    params.set(RELOAD_MARKER, '1');
    window.location.replace(`${window.location.pathname}?${params.toString()}${window.location.hash}`);
  }
}
