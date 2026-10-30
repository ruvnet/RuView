#!/usr/bin/env python3
"""Read-only smoke against the compiled server's actual HTTP listener."""
import argparse
import json
import os
import urllib.error
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--base-url', required=True)
args = parser.parse_args()
token = os.environ.get('RUVIEW_MAT_SMOKE_TOKEN')
if not token:
    raise SystemExit('Set RUVIEW_MAT_SMOKE_TOKEN for this isolated server; never pass credentials in arguments.')

def request(path, authenticated):
    headers = {'Authorization': 'Bearer ' + token} if authenticated else {}
    req = urllib.request.Request(args.base_url.rstrip('/') + path, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=5) as response:
            return response.status, json.load(response)
    except urllib.error.HTTPError as error:
        return error.code, None

for path in ['/api/v1/mat/events?page=0&page_size=100', '/api/v1/mat/scan/status', '/ws/mat/stream']:
    status, _ = request(path, False)
    assert status in (401, 403), (path, 'unauthenticated access was not rejected', status)
status, events = request('/api/v1/mat/events?page=0&page_size=100', True)
assert status == 200 and isinstance(events['events'], list), ('MAT HTTP event router', status)
assert events['page'] == 0 and events['page_size'] == 100
status, pipeline = request('/api/v1/mat/scan/status', True)
assert status == 200 and isinstance(pipeline['scanning'], bool), ('MAT HTTP pipeline router', status)
print(json.dumps({'result': 'PASS', 'evidence': 'EXECUTED_COMPILED_HTTP',
                  'eventCount': len(events['events']), 'mlEnabled': pipeline['ml_enabled'],
                  'mlReady': pipeline['ml_ready'], 'unauthenticatedReadsAndStream': 'denied'}))
