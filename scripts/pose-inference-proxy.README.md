# pose-inference-proxy

Sidecar that plumbs `model.safetensors` inference into the sensing server's
WebSocket broadcast so the dashboard renders real model output.

## Why

The sensing server accepts `--model <path>` but the runtime broadcast loop
emits `pose_keypoints: None` and keypoint confidences of `0.0` regardless of
whether a model is loaded. The loaded weights are read once at startup for
the `Layer A ready` log line and never fed into the broadcast path. This
script fills the gap as a sidecar so the wiring can be observed and the
dashboard can render real inference output without recompiling the Rust
crate.

## Run

```bash
pip install numpy safetensors websockets
huggingface-cli download ruvnet/wifi-densepose-pretrained \
  --local-dir models/wifi-densepose-pretrained

# 1. Start the sensing server normally
docker run -d -p 3000:3000 -p 3001:3001 ruvnet/wifi-densepose:latest

# 2. Start the proxy on a sibling port
python3 scripts/pose-inference-proxy.py \
  --model models/wifi-densepose-pretrained/model.safetensors \
  --upstream ws://localhost:3001/ws/sensing \
  --port 3002

# 3. Point the dashboard or any websocket client at the proxy port
#    ws://localhost:3002/ws/sensing
#    ws://localhost:3002/api/v1/stream/pose
```

## What changes in the stream

| Field | Upstream (port 3001) | Proxy (port 3002) |
|-------|----------------------|-------------------|
| `pose_keypoints` | `null` | 17 entries with real coordinates and presence sigmoid |
| `classification.confidence` | heuristic value | presence head sigmoid output |
| `persons[0].keypoints[*].confidence` | `0.0` | presence head sigmoid output |
| `__model_inference__` | absent | `{model, presence_confidence, embedding_norm}` |
| `pose_source` (on `/api/v1/stream/pose`) | `signal_derived` | `model_inference` |

## Caveats

* The published `model.safetensors` carries an encoder and a presence head.
  It does not carry a learned keypoint regressor. The proxy maps the
  embedding through a fixed Gaussian projection so the rendered skeleton
  reflects model state changes, but the per-joint positions are not from a
  trained pose head.
* The encoder accepts the sensing server's published simulation feed as
  well as real ESP32 CSI. Reported accuracy will reflect the input source.
* The script repairs the published safetensors header padding bug
  in-place on first load.
