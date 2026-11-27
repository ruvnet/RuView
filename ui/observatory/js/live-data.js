/** Live presence alone is not evidence of a measured pose. Demo figures remain illustrative. */
export function visiblePersons(data, live) {
  const persons = Array.isArray(data?.persons) ? data.persons : [];
  if (!live) return persons;
  return persons.filter(person => Array.isArray(person?.keypoints) &&
    person.keypoints.some(kp => Number.isFinite(kp?.confidence) && kp.confidence > 0));
}

/** A live frame older than this is no longer evidence of the room's current state. */
export const LIVE_STALE_MS = 5000;

/**
 * Decide what the Observatory is allowed to draw.
 *
 * Demo data is drawn only when demo was the selected source. In live mode the
 * page never substitutes demo frames: before the first frame, after a dropped
 * connection, or when frames stop arriving, it reports 'no-data' and draws an
 * empty room, so a fabricated person or vital sign can never appear to come
 * from a sensor.
 */
export function frameSource({ dataSource, liveFrame, liveAtMs, nowMs, staleMs = LIVE_STALE_MS }) {
  if (dataSource !== 'ws') return 'demo';
  if (!liveFrame || !Number.isFinite(liveAtMs) || nowMs - liveAtMs > staleMs) return 'no-data';
  return 'live';
}

/** Backoff for live reconnects: 1, 2, 4, 8 then 16 s, and it never gives up. */
export function reconnectDelayMs(attempt) {
  const n = Math.max(0, Math.floor(Number(attempt) || 0));
  return 1000 * 2 ** Math.min(n, 4);
}

/**
 * The banner every viewer sees while the picture is not live measurement, or
 * null when it is. `reason` says why demo is showing; `wsUrl` and `attempt`
 * describe the live connection being retried.
 */
export function sourceBanner(kind, { reason = '', wsUrl = '', attempt = 0 } = {}) {
  if (kind === 'demo') {
    return {
      tone: 'demo',
      title: 'DEMO: SIMULATED DATA',
      detail: 'Nothing on this screen comes from a sensor. People, vital signs and alerts are generated.' +
        (reason ? ` ${reason}` : ''),
    };
  }
  if (kind === 'no-data') {
    const where = wsUrl ? ` from ${wsUrl}` : '';
    return {
      tone: 'no-data',
      title: 'NO LIVE DATA',
      detail: `No frames${where} in the last ${LIVE_STALE_MS / 1000} s. Showing an empty room, not a guess.` +
        (attempt > 0 ? ` Reconnecting (attempt ${attempt}).` : ' Waiting for the sensing server.'),
    };
  }
  return null;
}

/**
 * The sensing server multiplexes other messages (e.g. per-node `edge_vitals`)
 * onto /ws/sensing. Only a sensing frame may replace the picture; anything
 * else would blank the classification and readings until the next frame.
 */
export function isSensingFrame(msg) {
  if (!msg || typeof msg !== 'object' || Array.isArray(msg)) return false;
  return msg.type === undefined || msg.type === 'sensing_update';
}
