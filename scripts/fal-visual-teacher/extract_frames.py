#!/usr/bin/env python3
"""Extract training stills from the real fal.ai room-visualization MP4s.

Reads the source videos (real fal.ai-generated room visualizations, one per
style preset) and writes JPEG frames + a caption .txt per frame into a local
(gitignored) training-data directory, ready to be zipped for fal.ai LoRA
training. Frame interval is time-based (not frame-count-based) so each
source contributes visual diversity proportional to its length.
"""

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# (source mp4 path, style tag, caption) -- sources are real fal.ai output,
# captured earlier this session by the fal-room-viz / room-viz-h3-upgrade
# agents. Caption describes the RuView visual style so the LoRA learns the
# recurring motifs (glowing pose overlay, WiFi signal arcs, vitals readout,
# dark tech aesthetic) rather than any single room layout.
SOURCES = [
    ("/tmp/room-viz-architectural/room-viz.mp4", "architectural",
     "ruview style room visualization, architectural wireframe overlay, "
     "glowing cyan pose skeleton, wifi signal arcs, dark tech aesthetic"),
    ("/tmp/room-viz-abstract/room-viz.mp4", "abstract",
     "ruview style room visualization, abstract particle field, "
     "glowing pose overlay, wifi signal arcs, dark tech aesthetic"),
    ("/tmp/room-viz-branded/room-viz.mp4", "branded",
     "ruview style room visualization, branded hud overlay, "
     "glowing cyan and emerald pose skeleton, vitals readout, dark tech aesthetic"),
    ("/tmp/room-viz-h3-architectural/room-viz.mp4", "h3-architectural",
     "ruview style room visualization, architectural wireframe overlay, "
     "glowing pose skeleton, wifi signal arcs, vitals readout, dark tech aesthetic"),
    ("/tmp/room-viz-h3-cinematic/room-viz.mp4", "h3-cinematic",
     "ruview style room visualization, cinematic lighting, "
     "glowing pose overlay, wifi signal arcs, vitals readout, dark tech aesthetic"),
    ("/tmp/room-viz-h3-minimal/room-viz.mp4", "h3-minimal",
     "ruview style room visualization, minimal clean overlay, "
     "glowing pose skeleton, subtle wifi signal arcs, dark tech aesthetic"),
    ("/tmp/room-viz-h3-abstract/room-viz.mp4", "h3-abstract",
     "ruview style room visualization, abstract particle field, "
     "glowing pose overlay, wifi signal arcs, dark tech aesthetic"),
]

FRAME_INTERVAL_SEC = 1.5


def extract(src: Path, style: str, caption: str, out_dir: Path) -> int:
    pattern = out_dir / f"{style}_%03d.jpg"
    cmd = [
        "ffmpeg", "-y", "-loglevel", "error",
        "-i", str(src),
        "-vf", f"fps=1/{FRAME_INTERVAL_SEC}",
        "-q:v", "2",
        str(pattern),
    ]
    subprocess.run(cmd, check=True)
    frames = sorted(out_dir.glob(f"{style}_*.jpg"))
    for frame in frames:
        frame.with_suffix(".txt").write_text(caption, encoding="utf-8")
    return len(frames)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out", default=str(REPO_ROOT / "scripts" / "fal-visual-teacher" / "training-data"),
        help="Output directory for extracted frames + captions (gitignored)",
    )
    args = parser.parse_args()

    out_dir = Path(args.out)
    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)

    total = 0
    for src_str, style, caption in SOURCES:
        src = Path(src_str)
        if not src.exists():
            print(f"SKIP (missing): {src}", file=sys.stderr)
            continue
        n = extract(src, style, caption, out_dir)
        print(f"{style}: {n} frames from {src}")
        total += n

    print(f"\nTotal frames extracted: {total}")
    print(f"Training data dir: {out_dir}")
    if total < 4:
        print("ERROR: fal.ai training needs at least 4 images", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
