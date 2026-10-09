# RuView README visual system

The README uses a Cognitum-inspired palette: midnight backgrounds, cyan gradients, wireframe geometry and restrained signal motion. It includes a hero, six feature cards, six navigation icons, an overview, five chapter headings and a Cognitum destination banner.

## Rebuild and check

```bash
python3 scripts/render-readme-visuals.py
python3 scripts/verify-readme-visuals.py
```

The generator uses only Python's standard library and makes no network calls. The committed SVGs are the deployment artifacts. The manifest in `assets/readme/manifest.json` lists every destination.

## Links and rendering

GitHub renders SVGs as images, so image-internal links are not used. Each SVG has an outer Markdown or HTML link in the README. Each card and navigation icon also has a plain text link for narrow screens and assistive technology. Source headings remain intact so existing README anchors still work.

Animation uses CSS only. All copy and essential geometry are visible without animation, JavaScript, remote fonts or remote images. `prefers-reduced-motion` disables decorative movement. The original GIF remains available in an expandable demo section; the existing ruOS promotion remains linked.

## Content boundaries

The overview and RF room are conceptual illustrations, not live measurements or benchmark evidence. Feature cards link to current source documentation. Pose research is identified as such, and the signal card directs readers to hardware validation. No new speed, accuracy, medical or device-support claims are introduced.

Sources: `docs/user-guide.md`, `docs/benchmarks/pose-estimation-cog.md`, `v2/crates/wifi-densepose-signal/README.md`, `firmware/esp32-csi-node/README.md`, `docs/integrations/home-assistant.md`, and `harness/ruview/README.md`.
