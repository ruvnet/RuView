#!/usr/bin/env python3
"""Run a real validation inference call against the trained visual-teacher LoRA.

Loads the trained model reference written by train_lora.py, calls
fal-ai/flux-lora with the trained LoRA weights, and saves the generated
image as evidence that training actually produced a working style LoRA.

This inference call, and the LoRA it exercises, produce SYNTHETIC display
imagery only (ADR-353) -- never a source of measured RF/pose/vitals/identity
or confidence values.

Requires FAL_KEY in the environment.
"""

import argparse
import json
import os
import sys
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_DIR = Path(__file__).resolve().parent
INFERENCE_ENDPOINT = "fal-ai/flux-lora"

DEFAULT_PROMPT = (
    "ruviewstyle room visualization, a living room with a glowing cyan pose "
    "skeleton overlay, wifi signal arcs radiating across the space, a vitals "
    "readout panel, dark tech aesthetic, synthetic sensor visualization"
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--training-result", default=str(SCRIPT_DIR / "output" / "training-result.json"),
    )
    parser.add_argument("--prompt", default=DEFAULT_PROMPT)
    parser.add_argument("--scale", type=float, default=1.0, help="LoRA application strength")
    parser.add_argument(
        "--out", default=str(REPO_ROOT / "assets" / "fal-visual-teacher-sample.png"),
    )
    args = parser.parse_args()

    if not os.environ.get("FAL_KEY"):
        print("ERROR: FAL_KEY not set in environment", file=sys.stderr)
        return 1

    import fal_client

    training_result_path = Path(args.training_result)
    record = json.loads(training_result_path.read_text(encoding="utf-8"))
    lora_url = record["result"]["diffusers_lora_file"]["url"]
    print(f"Using trained LoRA: {lora_url}")

    print(f"Submitting validation inference to {INFERENCE_ENDPOINT}...")
    result = fal_client.subscribe(
        INFERENCE_ENDPOINT,
        arguments={
            "prompt": args.prompt,
            "loras": [{"path": lora_url, "scale": args.scale}],
            "image_size": "landscape_16_9",
            "num_images": 1,
            "output_format": "png",
        },
        with_logs=True,
    )

    images = result.get("images", [])
    if not images:
        print(f"ERROR: no images in result: {result}", file=sys.stderr)
        return 1

    image_url = images[0]["url"]
    print(f"Generated image: {image_url}")

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    urllib.request.urlretrieve(image_url, out_path)
    print(f"Saved validation sample to {out_path}")

    meta_path = out_path.with_suffix(".json")
    meta_path.write_text(json.dumps({
        "endpoint": INFERENCE_ENDPOINT,
        "lora_url": lora_url,
        "prompt": args.prompt,
        "scale": args.scale,
        "source_image_url": image_url,
    }, indent=2), encoding="utf-8")
    print(f"Saved validation metadata to {meta_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
