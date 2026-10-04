#!/usr/bin/env python3
"""Live CSI->pose inference bridge (ADR-180).

Runs on the box with the live CSI. Loads the camera-supervised model (numpy,
no torch needed), subscribes to /ws/sensing, runs a forward pass per frame, and
broadcasts the predicted 17-keypoint pose to HTML clients on ws://:8770/pose.

  python wiflow_infer.py --model model/model.npz \
      --in ws://localhost:8765/ws/sensing --port 8770
"""
import argparse, asyncio, json, os
import numpy as np
import websockets

# COCO skeleton edges (for the client; sent once in 'meta')
EDGES = [[5,7],[7,9],[6,8],[8,10],[5,6],[11,12],[5,11],[6,12],
         [11,13],[13,15],[12,14],[14,16],[0,1],[0,2],[1,3],[2,4],[0,5],[0,6]]

def csi_vector(frame):
    f = frame.get("features", {}) or {}
    feats = [f.get("mean_rssi",0.0), f.get("variance",0.0),
             f.get("motion_band_power",0.0), f.get("breathing_band_power",0.0)]
    pernode = {nf.get("node_id"): (nf.get("features") or {}) for nf in (frame.get("node_features") or [])}
    for nid in (9,13):
        nf = pernode.get(nid,{}); feats += [nf.get("mean_rssi",0.0), nf.get("variance",0.0), nf.get("motion_band_power",0.0)]
    field = (frame.get("signal_field",{}) or {}).get("values") or []
    field = (field + [0.0]*400)[:400]
    return np.array(feats + field, np.float32)

class Model:
    def __init__(self, path):
        z = np.load(path)
        self.mu, self.sd = z["mu"], z["sd"]
        self.W = [z["net_0_weight"], z["net_3_weight"], z["net_6_weight"], z["net_8_weight"]]
        self.b = [z["net_0_bias"],   z["net_3_bias"],   z["net_6_bias"],   z["net_8_bias"]]
    def __call__(self, x):
        h = (x - self.mu) / self.sd
        for i in range(3):
            h = np.maximum(0.0, h @ self.W[i].T + self.b[i])     # Linear+ReLU
        out = 1.0/(1.0+np.exp(-(h @ self.W[3].T + self.b[3])))   # Linear+Sigmoid -> 34
        return out.reshape(17,2)

def vitals_fields(frame):
    """Forward REAL vitals from the sensing frame into the /pose payload, honestly.

    The sensing server (csi.rs) computes an FFT breathing-rate estimate and emits
    ``features.dominant_freq_hz`` / ``features.breathing_band_power``, plus gated
    ``breathing_rate_bpm`` / ``heart_rate_bpm`` on a ``vitals`` object when present.

    - ``bpm_breath``: an explicit ``breathing_rate_bpm`` if the pipeline reports one,
      else derived from the breathing-band spectral peak (``dominant_freq_hz`` inside
      the 0.1-0.5 Hz band -> bpm = f*60). Omitted when there is no in-band signal.
    - ``bpm_heart``: forwarded ONLY when the pipeline explicitly reports a plausible
      value (40-180 bpm). WiFi heart-rate is never synthesised here.
    - ``stress``: not produced by the pipeline, so it is never emitted (the HUD keeps
      its own clearly-labelled synthetic stress trace).

    Every field is an *estimate* from real signal; the client already flags coarse
    accuracy. Fields are simply left off when unavailable, so the HUD shows ``sim``.
    """
    out = {}
    vit = frame.get("vitals") or {}
    feats = frame.get("features") or {}

    br = vit.get("breathing_rate_bpm") or frame.get("breathing_rate_bpm")
    if not br:
        f = feats.get("dominant_freq_hz")
        band_power = feats.get("breathing_band_power") or 0.0
        if f is not None and 0.1 <= f <= 0.5 and band_power > 0.0:
            br = f * 60.0
    if br and 5.0 <= br <= 35.0:
        out["bpm_breath"] = round(float(br), 1)

    hr = vit.get("heart_rate_bpm") or frame.get("heart_rate_bpm")
    if hr and 40.0 <= hr <= 180.0:
        out["bpm_heart"] = round(float(hr), 1)

    return out

CLIENTS = set()
LATEST = {"pose": None}

async def serve_client(ws):
    CLIENTS.add(ws)
    try:
        await ws.send(json.dumps({"type":"meta","edges":EDGES}))
        async for _ in ws:    # client is read-only; just keep alive
            pass
    except Exception:
        pass
    finally:
        CLIENTS.discard(ws)

async def infer_loop(model, in_url):
    while True:
        try:
            async with websockets.connect(in_url, open_timeout=8, ping_interval=20) as ws:
                async for msg in ws:
                    d = json.loads(msg)
                    kp = model(csi_vector(d))
                    cls = d.get("classification",{})
                    payload = {"type":"pose","src":d.get("source"),
                               "presence":bool(cls.get("presence")),
                               "motion":(d.get("features",{}) or {}).get("motion_band_power"),
                               "kps":[[round(float(x),4),round(float(y),4)] for x,y in kp],
                               "nodes":sorted(n.get("node_id") for n in d.get("nodes",[]) if n.get("node_id") is not None)}
                    payload.update(vitals_fields(d))   # real breathing/heart when the feed carries them
                    # IPS fields: forward-only, never synthesised here. `ranges`
                    # is [{node_id, r_m}] and `target` {x,y,z} in the blueprint
                    # frame; the HUD runs its own multilateration on `ranges`.
                    for key in ("target", "ranges", "target_true"):
                        if key in d:
                            payload[key] = d[key]
                    LATEST["pose"]=payload
                    if CLIENTS:
                        dead=[]
                        for c in list(CLIENTS):
                            try: await c.send(json.dumps(payload))
                            except Exception: dead.append(c)
                        for c in dead: CLIENTS.discard(c)
        except Exception as e:
            print(f"[infer] reconnect ({e})", flush=True); await asyncio.sleep(1.0)

async def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default=os.path.join(os.path.dirname(__file__),"model","model.npz"))
    ap.add_argument("--in", dest="in_url", default="ws://localhost:8765/ws/sensing")
    ap.add_argument("--port", type=int, default=8770)
    args = ap.parse_args()
    model = Model(args.model)
    print(f"[infer] model {args.model} loaded; serving predicted poses on ws://0.0.0.0:{args.port}/pose")
    async with websockets.serve(serve_client, "0.0.0.0", args.port):
        await infer_loop(model, args.in_url)

if __name__ == "__main__":
    asyncio.run(main())
