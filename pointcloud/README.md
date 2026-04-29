# RuView — Live 3D Point Cloud Viewer

Hosted at: https://ruvnet.github.io/RuView/pointcloud/

## Modes

- Default — synthetic in-browser demo (no backend, no network calls).
- `?backend=auto` — fetch from `/api/splats` on the same origin
  (only works when the viewer is served by `ruview-pointcloud serve`).
- `?backend=<url>` — fetch from `<url>/api/splats` on a CORS-permitting
  host (e.g. `?backend=https://my-ruview.example.com`).
- `?live=1` — require a live backend; show an offline message instead
  of falling back to the synthetic demo.

See ADR-094 for the deployment design.
