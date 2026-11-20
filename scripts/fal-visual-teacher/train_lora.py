#!/usr/bin/env python3
"""Submit a real fal.ai LoRA training job for the RuView synthetic visual teacher.

Zips the extracted training-data directory, uploads it to fal.ai storage,
submits a training job to fal-ai/flux-lora-fast-training, polls until it
completes, and writes the resulting trained-model reference (diffusers LoRA
weights URL + config URL) to a local (gitignored) JSON file.

Architecture boundary (ADR-353): the model trained here is ONLY ever a
source of supplementary synthetic visual/pixel output. It must never
influence, or be treated as a source of, measured/derived RF, pose, vitals,
identity, or confidence values.

Requires FAL_KEY in the environment (fetch from GCP Secret Manager,
project cognitum-20260110, secret MINIMAX_MUSIC_FAL_KEY, before running).
"""

import argparse
import json
import os
import sys
import time
import zipfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
TRAINING_ENDPOINT = "fal-ai/flux-lora-fast-training"


def zip_training_data(src_dir: Path, zip_path: Path) -> None:
    if zip_path.exists():
        zip_path.unlink()
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as zf:
        for f in sorted(src_dir.iterdir()):
            if f.is_file():
                zf.write(f, arcname=f.name)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--training-data", default=str(SCRIPT_DIR / "training-data"),
        help="Directory of extracted frames + captions (from extract_frames.py)",
    )
    parser.add_argument(
        "--steps", type=int, default=1000,
        help="Training steps (fal-ai/flux-lora-fast-training default is 1000, ~$2)",
    )
    parser.add_argument(
        "--trigger-word", default="ruviewstyle",
        help="Trigger token to prepend so the LoRA is invocable at inference time",
    )
    parser.add_argument(
        "--out", default=str(SCRIPT_DIR / "output" / "training-result.json"),
        help="Where to write the trained model reference JSON (gitignored)",
    )
    args = parser.parse_args()

    if not os.environ.get("FAL_KEY"):
        print("ERROR: FAL_KEY not set in environment", file=sys.stderr)
        return 1

    import fal_client  # imported after the FAL_KEY check for a clearer error

    training_dir = Path(args.training_data)
    if not training_dir.exists():
        print(f"ERROR: {training_dir} not found -- run extract_frames.py first", file=sys.stderr)
        return 1

    zip_path = training_dir.parent / "training-data.zip"
    print(f"Zipping {training_dir} -> {zip_path}")
    zip_training_data(training_dir, zip_path)
    print(f"Zip size: {zip_path.stat().st_size / 1024:.1f} KB")

    print("Uploading training zip to fal.ai storage...")
    images_data_url = fal_client.upload_file(str(zip_path))
    print(f"Uploaded: {images_data_url}")

    training_args = {
        "images_data_url": images_data_url,
        "steps": args.steps,
        "trigger_word": args.trigger_word,
        # This is a style LoRA (recurring visual aesthetic across varied room
        # layouts), not a subject/character LoRA -- fal.ai's is_style flag
        # disables segmentation/auto-captioning so our own caption .txt files
        # (one per frame, describing the shared style) are used as-is.
        "is_style": True,
    }
    print(f"Submitting training job to {TRAINING_ENDPOINT} (steps={args.steps})...")
    handle = fal_client.submit(TRAINING_ENDPOINT, arguments=training_args)
    print(f"request_id: {handle.request_id}")

    start = time.time()
    for event in handle.iter_events(with_logs=True, interval=5.0):
        print(f"[{time.time() - start:.0f}s] {event}")

    result = handle.get()
    elapsed = time.time() - start
    print(f"Training completed in {elapsed:.0f}s")

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    record = {
        "endpoint": TRAINING_ENDPOINT,
        "request_id": handle.request_id,
        "steps": args.steps,
        "trigger_word": args.trigger_word,
        "training_images": sum(1 for f in training_dir.glob("*.jpg")),
        "elapsed_seconds": round(elapsed),
        "result": result,
    }
    out_path.write_text(json.dumps(record, indent=2), encoding="utf-8")
    print(f"Wrote {out_path}")

    lora_file = (result or {}).get("diffusers_lora_file", {}).get("url")
    if lora_file:
        print(f"\nTrained LoRA weights URL: {lora_file}")
    else:
        print("\nWARNING: no diffusers_lora_file.url in result -- inspect the JSON", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
