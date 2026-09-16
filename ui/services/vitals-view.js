// Presentation rules for the vitals panel.
//
// Why this is a separate, dependency-free module: the reviewer of the live-view
// PR rejected an earlier version of this panel because a numeric vitals value
// stayed on screen after the governing gate closed (`Verdict: REJECT`, "stale
// numeric vitals remain displayed after the governed frame abstains/room becomes
// empty"). The rule that decides what may be shown as *current* is therefore worth
// testing on its own, without a DOM or a socket.
//
// The gate publishes a value only when a fresh explicit calibration, exactly one
// occupant and qualified confidence all hold at that instant, and it publishes on
// a minority of frames (MEASURED ~9.5%). Two failure modes have to be avoided at
// once:
//   * flicker: clearing on every frame that lacks a value hides numbers the server
//     is in fact publishing, and made the operator think the feature was broken;
//   * a stale claim: showing an old number as if it were current is worse, because
//     it presents an expired authorization as a live measurement.
//
// So: a value is shown as CURRENT only while the governing frame still carries
// authority *and* it is young. Once authority closes, the numeric fields are
// cleared and the last value is downgraded to an explicitly historical line that
// says when it was published.

/** A value is current while it is at most this old and its frame carried authority. */
export const VITALS_CURRENT_GRACE_S = 2;

/** The historical line is kept this long, then dropped entirely. */
export const VITALS_HISTORY_S = 120;

function fmt(value, unit, digits) {
  return (typeof value === 'number' && isFinite(value))
    ? `${value.toFixed(digits)} ${unit}`
    : null;
}

/**
 * @param {object} input
 * @param {{breathing_rate_bpm?: number|null, heart_rate_bpm?: number|null}|null} input.vitals
 *   the most recent published vitals, or null if none has been published yet
 * @param {number|null} input.ageSeconds  age of that publication, or null
 * @param {boolean} input.authorityOpen   did the most recent governed frame carry
 *   a published value?
 * @param {string|null} input.reason      the server's abstention reason, for the note
 */
export function vitalsView({ vitals, ageSeconds, authorityOpen, reason }) {
  const breathing = vitals ? fmt(vitals.breathing_rate_bpm, 'rpm', 1) : null;
  const heart = vitals ? fmt(vitals.heart_rate_bpm, 'bpm', 0) : null;
  const hasAny = Boolean(breathing || heart);
  const age = (typeof ageSeconds === 'number' && isFinite(ageSeconds)) ? ageSeconds : null;
  const young = age !== null && age <= VITALS_CURRENT_GRACE_S;

  const historical = (hasAny && age !== null && age <= VITALS_HISTORY_S)
    ? { breathing, heart, ageSeconds: age,
        text: [breathing, heart].filter(Boolean).join('  \u00b7  ') + `  \u00b7  ${age}s ago` }
    : null;

  if (!hasAny) {
    return {
      state: 'none',
      breathing: 'abstained',
      heart: 'abstained',
      note: reason || 'needs a fresh calibration and exactly one occupant',
      historical: null,
    };
  }

  if (authorityOpen && young) {
    return {
      state: 'current',
      breathing: breathing || 'abstained',
      heart: heart || 'abstained',
      note: 'published by the sensing server',
      historical: null,
    };
  }

  // Authority closed, or the publication is no longer young: nothing numeric may be
  // presented as current. The last value is downgraded to an explicitly historical
  // line rather than left in the current field.
  return {
    state: 'withheld',
    breathing: 'withheld',
    heart: 'withheld',
    note: authorityOpen
      ? `no publication for ${age}s`
      : (reason || 'authority closed for the governed frame'),
    historical,
  };
}
