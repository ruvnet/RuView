# RuView README visual system

The README uses a Cognitum-inspired palette: midnight backgrounds, cyan gradients, wireframe geometry and restrained signal motion. It includes a hero, six feature cards, six navigation icons, an overview, five chapter headings and a Cognitum destination banner.

## Rebuild and check

```bash
python3 -m pip install -r scripts/requirements-readme-visuals.txt
python3 scripts/render-readme-visuals.py
python3 scripts/verify-readme-visuals.py
python3 -m unittest discover -s scripts/tests -p test_readme_visuals_xml.py
```

Install the tooling dependency in a virtual environment. The generator and verifier
use `defusedxml` to reject DTDs, entities and external XML references. SVG inputs
must be smaller than 20,000 bytes; the verifier reads at most that limit and
rejects oversized inputs before parsing. Both tools use UTF-8, and generated
files use LF line endings for reproducibility across platforms. They make no
network calls. This dependency is only for visual maintenance, not the runtime.
Manifest paths must be relative: SVG files stay inside `assets/readme`, and
local link targets stay inside the repository, including after symlink resolution.

The committed SVGs are the deployment artifacts. The manifest in
`assets/readme/manifest.json` lists every destination.

## Links and rendering

GitHub renders SVGs as images, so image-internal links are not used. Each SVG has an outer Markdown or HTML link in the README. Each card and navigation icon also has a plain text link for narrow screens and assistive technology. Source headings remain intact so existing README anchors still work.

Animation uses CSS only. All copy and essential geometry are visible without animation, JavaScript, remote fonts or remote images. `prefers-reduced-motion` disables decorative movement. The original GIF remains available in an expandable demo section; the existing ruOS promotion remains linked.

## Content boundaries

The overview and RF room are conceptual illustrations, not live measurements or benchmark evidence. Feature cards link to current source documentation. Pose research is identified as such, and the signal card directs readers to hardware validation. No new speed, accuracy, medical or device-support claims are introduced.

Sources: `docs/user-guide.md`, `docs/benchmarks/pose-estimation-cog.md`, `v2/crates/wifi-densepose-signal/README.md`, `firmware/esp32-csi-node/README.md`, `docs/integrations/home-assistant.md`, and `harness/ruview/README.md`.
