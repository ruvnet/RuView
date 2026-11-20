# Virtual-view broker adapter (research prototype)

Opt-in integration with Dream Machine's `scripts/virtual-view-broker.mjs`.
Disabled by default. No cameras, model calls, network, hardware commands or
automatic promotion. It is NOT part of the Rust production perception pipeline.

The reviewed companion is Dream Machine commit
[`5707c825b2c065de44abed02594e1abf5eab95ea`](https://github.com/ruvnet/dream-machine/tree/5707c825b2c065de44abed02594e1abf5eab95ea),
merged in [Dream Machine PR #162](https://github.com/ruvnet/dream-machine/pull/162).
Use a checkout at that revision when reproducing the integration.

```js
import { generateKeyPairSync } from 'node:crypto';
import { broker } from '../../../dream-machine/scripts/virtual-view-broker.mjs';
import { createVirtualViewAdapter } from './adapter.mjs';
// Example in-memory research identity only; use a trusted host signer in deployment.
const { privateKey } = generateKeyPairSync('ed25519');
const adapter = createVirtualViewAdapter({ broker, privateKey, enabled: true });
// adapter.observe(calibratedSnapshot) returns RENDER, SKIP or BLOCK plus authority:none.
```

The example assumes sibling RuView and Dream Machine clones; adjust the import
from the executing module's location. Pass calibrated `virtual-view.v1` scene
snapshots in room-local ENU metres. See Dream Machine ADR-0108 for every field,
threshold, trust boundary, receipt and acceptance rule. The adapter does not
transform raw `spatial.evidence.v1` RF Gaussians or `ruview.lidar.depth.v1` packets:
neither supplies the independently tracked manipulation scene this method needs.

The injected broker is trusted executable code, not sandboxed by this adapter.
The no-network/no-actuation behavior applies to this adapter and the reviewed
companion broker. Before reading scene markers or calling the broker, the adapter
checks the entire snapshot as bounded plain JSON using property descriptors.
It rejects proxies, getters, inherited/custom prototypes, nonfinite values,
sparse arrays and non-JSON data, including nested values. Structural limits match
the reviewed companion: depth 12, 20,000 values, 1,024 array elements, 64 object
properties, 128-character keys and 4,096-character strings. Full geometry,
provenance-label and signing-key validation remain the companion broker's responsibility.

```sh
node --test integrations/virtual-view/adapter.test.mjs
# From the sibling Dream Machine checkout:
node scripts/bench-virtual-view.mjs ../ruview/integrations/virtual-view/adapter.mjs
```

The cross-repository benchmark exercises this adapter with the real broker,
renders 256-object synthetic scenes, signs and replays receipts. It measures local
CPU overhead only. Required robot evidence remains missing: +20 points on two of
three held-out tasks, no regression beyond five points, added p95 below 150 ms,
at least 95% pre-action drift detection, and replayable receipts for all trials.
No receipt grants actuator authority. Do not wire this to live robot control.
