"""Tiny threaded HTTP server for the three.js demos that fetch local files.

Why a sibling helper script instead of `python -m http.server`?
The stdlib SimpleHTTPServer is single-threaded; Chrome opens many parallel
connections (HTML + 9 script tags + FBX), the first eats the worker, the
rest time out with net::ERR_EMPTY_RESPONSE. ThreadingHTTPServer fixes it.

Usage:
    python examples/three.js/server/serve-demo.py
    open http://localhost:8765/examples/three.js/demos/05-skinned-realtime.html
"""
from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler
import os, sys

PORT = int(os.environ.get("PORT", 8765))
# Always serve from the repo root regardless of where the script is launched.
# This file lives at examples/three.js/server/serve-demo.py — three levels deep.
os.chdir(os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..")))

class NoCacheHandler(SimpleHTTPRequestHandler):
    def end_headers(self):
        # Aggressive no-cache so browser ALWAYS fetches the latest .html
        # after we edit it. Otherwise stale code sticks around even on hard
        # refresh and you debug a phantom.
        self.send_header("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        super().end_headers()

DEMOS = [
    "01-helpers.html",
    "02-cinematic.html",
    "03-skinned.html",
    "04-skinned-fbx.html",
    "05-skinned-realtime.html",
]

with ThreadingHTTPServer(("127.0.0.1", PORT), NoCacheHandler) as srv:
    print(f"serving {os.getcwd()} on http://127.0.0.1:{PORT}/")
    print("demos:")
    for d in DEMOS:
        print(f"  http://127.0.0.1:{PORT}/examples/three.js/demos/{d}")
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        sys.exit(0)
