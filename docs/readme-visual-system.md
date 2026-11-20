# RuView README visual system

The README matches the RuView header palette: deep navy backgrounds, electric blue
RF fields, lime pose and signal accents, warm amber joints and soft white text,
layered isometric geometry, moving scan planes, radar sweeps, articulated pose
limbs, breathing curves and separated hardware layers. Each feature uses its own
illustration rather than repeating the same wireframe cube. It includes a hero, six feature cards, six navigation icons, an overview,
five chapter headings and a Cognitum destination banner. Headlines use a heavier
Avenir Next / Segoe UI Variable Display / Segoe UI / Helvetica stack, with tighter spacing and a dedicated
system monospace stack for labels. Fonts remain local fallbacks so the images
also render without a network connection.

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
Spatial artwork uses `-spatial-v4.svg` filenames so GitHub's image proxy requests
fresh content after the visual upgrade. Previous filenames remain available for
existing links; new README wrappers use the versioned assets.

## Links and rendering

GitHub renders SVGs as images, so image-internal links are not used. Each SVG has an outer Markdown or HTML link in the README. Each card and navigation icon also has a plain text link for narrow screens and assistive technology. Source headings remain intact so existing README anchors still work.

Animation uses CSS only. Depth is projected directly into SVG geometry rather
than relying on a WebGL canvas or CSS perspective. Translation is applied to
inner groups so it cannot overwrite the outer icon placement. Projected orbit planes stay fixed while dashed signals move along them; scan
planes sweep the wireframe volumes. Unused animation rules are removed from
each image to keep the complete active SVG set below 100 KB.
All copy and essential geometry are visible without animation, JavaScript,
remote fonts or remote images. `prefers-reduced-motion` disables decorative
movement. The original GIF remains at the top; the existing ruOS promotion
remains linked. Existing asset dimensions and README links are preserved.

## Content boundaries

The overview and RF room are conceptual illustrations, not live measurements or benchmark evidence. Feature cards link to current source documentation. Pose research is identified as such, and the signal card directs readers to hardware validation. No new speed, accuracy, medical or device-support claims are introduced.

Sources: `docs/user-guide.md`, `docs/benchmarks/pose-estimation-cog.md`, `v2/crates/wifi-densepose-signal/README.md`, `firmware/esp32-csi-node/README.md`, `docs/integrations/home-assistant.md`, and `harness/ruview/README.md`.

The hero and pose card use `-spatial-v6.svg`: an original anatomical human
silhouette with shaded torso, head, hands and feet, overlapping limbs, surface
contours and amber joint markers. A 1.6-second gait drives opposing arm and leg swings, weight shifts and ground
shadow compression. An 8-second CSS perspective turn and moving surface light
give the anatomical illustration depth. Each limb pivots at its own shoulder
or hip. The effect remains a projected SVG illustration, not a skinned WebGL
model. The stylesheet disables gait and perspective movement for reduced motion.
Chrome 154 validation confirmed the 3D transform, opposing limb transforms,
animation in an HTML image, and a frozen standalone SVG under reduced motion.
Reduced-motion emulation did not freeze the embedded image in that test; image
preference propagation remains a browser-dependent limitation. This is vector artwork, not a real body scan or
measured pose reconstruction. Other assets retain their v4 filenames.
