# Companion spatial artifact v2

This extends explicit saved-scan transfer with bounded RoomPlan objects and stable published-room identity. It does not register coordinates, activate calibration, train a model, export images/depth, or grant a receive permission.

## Wire compatibility

The existing granted GET /companion/v1/spatial ticket advertises supportedArtifactSchemas containing ruview.companion.spatial-artifact.v1 and ruview.companion.spatial-artifact.v2. The nonce and expiry rules are unchanged. A phone sends v2 only when advertised. A phone with objects must refuse an older helper rather than silently discard them; a surface-only scan may retain the v1 path.

V2 retains every v1 field and requires roomId plus objects. roomId must match the ID of the currently published room as well as its installation, revision and binding digest at upload and acceptance. Objects is an array of at most128 records with exactly id, category, confidence, dimensionsM and transform. Identifiers are bounded; dimensions are finite/nonnegative; transforms are bounded rigid proper transforms; duplicate object IDs are rejected. Null/missing object arrays, v1 object/roomId extensions, unknown fields and payloads beyond the existing48KiB cap are rejected. Original JSON bytes determine the digest and remain immutable.

A matching synthetic v2 JSON fixture is executed by both app and server tests. It contains a floor and chair, no device/user data.

## Durable history and rollback

V2-capable helpers write accepted-spatial-artifacts-v2.json with spatial-history.v2 metadata. The original v1 archive is read and validated but never rewritten. New history merges both archives by digest, sorts by acceptance time, and preserves the existing bounded history limit. This includes additional v1 scans accepted after rollback. An older helper continues reading its unchanged v1 archive; v2-only scans are unavailable there and reappear on upgrade. Both archives remain account/installation scoped and permission/symlink/digest checked. A corrupt/foreign archive is preserved and refused, never silently reset.

The v2 archive must remain inside the same account-owned companion data directory, including existing account-erasure/backup coverage. Deploy the helper capability before expecting object uploads. Old apps may show v2 as unreadable while preserving original receipts; they must never reinterpret them as v1. No migration or installed-helper replacement is performed by the source PR.

## Rendering

Objects remain in the scan's captured historical frame. The app shows object count and outlines and may render the immutable accepted geometry after its existing owner/helper/digest/room custody checks. Objects and surfaces have independent display switches. Historical objects never become live detections, tracks, RF presence evidence, calibration anchors or model authority.
