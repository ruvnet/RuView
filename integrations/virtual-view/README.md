# Virtual-view broker adapter (research prototype)

Opt-in integration with Dream Machine's `scripts/virtual-view-broker.mjs`.
Disabled by default. No cameras, model calls, network, hardware commands or
automatic promotion. It is NOT part of the Rust production perception pipeline.

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
