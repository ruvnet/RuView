#!/usr/bin/env python3
"""Verify the generated SVGs and GitHub-compatible image link wrappers."""
from pathlib import Path
import json,re,xml.etree.ElementTree as ET
from urllib.parse import urlparse
ROOT=Path(__file__).resolve().parents[1];NS='{http://www.w3.org/2000/svg}'
manifest=json.loads((ROOT/'assets/readme/manifest.json').read_text());readme=(ROOT/'README.md').read_text();total=0
for asset in manifest['assets']:
 file=ROOT/asset['file'];raw=file.read_text();total+=file.stat().st_size;root=ET.fromstring(raw)
 assert root.tag==NS+'svg' and root.get('viewBox'),asset['file']
 assert root.find(NS+'title') is not None and root.find(NS+'desc') is not None
 ids=[e.get('id') for e in root.iter() if e.get('id')];assert len(ids)==len(set(ids))
 assert 'prefers-reduced-motion:reduce' in raw and '@keyframes' in raw
 for e in root.iter():
  assert e.tag not in [NS+'script',NS+'foreignObject',NS+'image'],asset['file']
  assert all(not k.lower().startswith('on') for k in e.attrib)
  assert not any(k.endswith('href') for k in e.attrib),'Links belong to README wrappers'
 assert not re.search(r'url\((?!#)',raw),'Remote dependency'
 target=asset['target'];parsed=urlparse(target)
 if parsed.scheme:
  assert parsed.scheme=='https' and parsed.hostname in ['cognitum.one','huggingface.co']
 else:assert (ROOT/target.split('#')[0]).is_file(),target
 escaped=re.escape(asset['file']);destination=re.escape(target)
 linked=bool(re.search(r'\[!\[[^\]]*\]\('+escaped+r'\)\]\('+destination+r'\)',readme))
 linked=linked or bool(re.search(r'<a href="'+destination+r'">\s*<img src="'+escaped+r'"',readme))
 assert linked,('Missing clickable wrapper',asset['file'])
 assert file.stat().st_size<20000,'Unexpected asset growth'
assert total<100000
print(f'PASS: {len(manifest["assets"])} accessible animated SVGs, link targets and README wrappers; {total:,} bytes. No scripts, remote resources or SVG-internal links.')
