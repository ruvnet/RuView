"""Explicit local C11 build; no downloads, model writes or implicit compilation."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

FLAGS=['-std=c11','-O3','-fPIC','-shared','-fno-fast-math','-ffp-contract=off']


def build(output):
    if sys.platform!='linux': raise ValueError('Linux build required')
    compiler=shutil.which('cc')
    if compiler is None: raise RuntimeError('System cc is required; no compiler is downloaded')
    root=Path(output).resolve()
    root.mkdir(parents=True,exist_ok=False)
    def write(name,value): (root/name).write_text(json.dumps(value,indent=2)+'\n')
    write('status.json',{'status':'running','promotionAuthorized':False})
    try:
        source=Path(__file__).with_suffix('.c')
        source=source.with_name('espfi_motion_native.c')
        source_bytes=source.read_bytes()
        # Compile an exclusive snapshot so the receipt describes the actual input.
        snapshot=root/'kernel.c';snapshot.write_bytes(source_bytes)
        version=subprocess.run([compiler,'--version'],check=True,capture_output=True,text=True,timeout=30).stdout
        command=[compiler,*FLAGS,str(snapshot),'-o',str(root/'kernel.so'),'-lm']
        result=subprocess.run(command,check=True,capture_output=True,text=True,timeout=60)
        library=(root/'kernel.so').read_bytes()
        if not 0<len(library)<=16*1024**2: raise ValueError('Unexpected kernel size')
        report={'status':'complete','promotionAuthorized':False,'abi':1,'compiler':compiler,
                'compiler_version':version[:8192],'flags':FLAGS+['-lm'],
                'source_sha256':hashlib.sha256(source_bytes).hexdigest(),
                'library_sha256':hashlib.sha256(library).hexdigest(),
                'library_bytes':len(library),'build_implementation_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
        write('build.json',report);write('status.json',{'status':'complete','promotionAuthorized':False})
        return report
    except Exception:
        write('status.json',{'status':'failed','promotionAuthorized':False})
        raise


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',required=True)
    print(json.dumps(build(parser.parse_args().output),indent=2))

if __name__=='__main__':main()
