"""Lossless seed storage and measured float64 folding for the retained ACF model."""
import argparse
import hashlib
import json
from pathlib import Path
import time
import numpy as np
from safetensors.numpy import load, save_file
from .espfi_motion import MotionBundle, _read, _validate, motion_features
from .espfi_kernel import predict_features
from .espfi_train import digest, write_json

BASELINE = dict(family='motion_acf', kind='rff', dimensions=128, gamma_multiplier=.1, alpha=.1)
SHAPES = {'feature_mean': (468,), 'feature_scale': (468,), 'output_mean': (128,),
          'weights': (128, 7), 'bias': (7,)}
SPEC = {'candidate': BASELINE, 'seed': 42, 'rng': 'numpy.default_rng.PCG64',
        'recipe': 'normal(0,sqrt(2*.1/468),(468,128)), then uniform(0,2*pi,128)',
        'precision': 'float64', 'features': 'unchanged motion_features motion_acf',
        'max_abs_logit_error': 1e-10, 'promotionAuthorized': False}


def random_map():
    rng = np.random.default_rng(42)
    return {'projection': rng.normal(0, np.sqrt(2 * (.1 / 468)), (468, 128)),
            'phase': rng.uniform(0, 2*np.pi, 128)}


def tensor_hash(value):
    return hashlib.sha256(np.ascontiguousarray(value).tobytes()).hexdigest()


def validate(state):
    if (set(state) != set(SHAPES) or any(state[k].shape != shape or state[k].dtype != np.float64
            or not np.isfinite(state[k]).all() for k, shape in SHAPES.items())
            or not (state['feature_scale'] > 0).all()):
        raise ValueError('Invalid compact float64 ACF state')


class CompactMotionBundle:
    def __init__(self, directory, folded=True):
        root = Path(directory)
        status = json.loads(_read(root/'status.json', 65536))
        if not isinstance(status, dict) or status.get('status') != 'complete' or status.get('promotionAuthorized') is not False:
            raise ValueError('Incomplete compact ACF artifact')
        metadata = json.loads(_read(root/'metadata.json', 65536))
        if (not isinstance(metadata, dict) or metadata.get('spec') != SPEC or metadata.get('status') != 'complete'
                or metadata.get('promotionAuthorized') is not False):
            raise ValueError('Invalid compact ACF metadata')
        data = _read(root/'model.safetensors', 65536)
        if hashlib.sha256(data).hexdigest() != metadata.get('model_sha256'):
            raise ValueError('Compact model hash mismatch')
        compact = load(data)
        validate(compact)
        generated = random_map()
        if {k: tensor_hash(v) for k,v in generated.items()} != metadata.get('random_map_sha256'):
            raise ValueError('Regenerated map differs from export')
        self._initialize(compact, generated, folded)

    def _initialize(self, compact, generated, folded):
        self.folded = folded
        if folded:
            projection = generated['projection'] / compact['feature_scale'][:, None]
            self.projection = np.ascontiguousarray(projection)
            self.phase = generated['phase'] - compact['feature_mean'] @ projection
            self.bias = compact['bias'] - compact['output_mean'] @ compact['weights']
            self.weights = compact['weights']
            if not all(np.isfinite(v).all() for v in (self.projection,self.phase,self.bias,self.weights)):
                raise ValueError('Nonfinite folded state')
        else:
            self.state = dict(compact, **generated)

    def predict(self, values):
        features = motion_features(values, 'motion_acf')
        if not self.folded:
            return predict_features(self.state, features)
        z = np.sqrt(2./128) * np.cos(features @ self.projection + self.phase)
        result = z @ self.weights + self.bias
        if not np.isfinite(result).all():
            raise ValueError('Nonfinite compact inference')
        return result


def export(parts, provenance, baseline_directory, output, max_seconds=600):
    if not 1 <= max_seconds <= 3600:
        raise ValueError('Invalid time budget')
    _validate(parts)
    baseline = MotionBundle(baseline_directory)
    prior_bytes = _read(Path(baseline_directory)/'metrics.json',16*1024**2)
    prior = json.loads(prior_bytes)
    if (baseline.candidate != BASELINE or not provenance.get('manifest_sha256')
            or prior.get('provenance',{}).get('manifest_sha256') != provenance['manifest_sha256']):
        raise ValueError('Baseline candidate or provenance mismatch')
    root = Path(output)
    root.mkdir(parents=True, exist_ok=False)
    write_json(root/'status.json',{'status':'running','promotionAuthorized':False})
    start = time.monotonic()
    try:
        generated = random_map()
        if any(not np.array_equal(baseline.state[k],generated[k]) for k in generated):
            raise ValueError('Baseline projection differs from frozen seed recipe')
        compact = {k:np.ascontiguousarray(baseline.state[k]) for k in SHAPES}
        validate(compact)
        save_file(compact,str(root/'model.safetensors'))
        # Validate the exact serialized bytes before constructing either runtime.
        replay = load(_read(root/'model.safetensors',65536))
        validate(replay)
        if any(not np.array_equal(replay[k],compact[k]) for k in SHAPES):
            raise ValueError('Compact export changed state')
        runtimes = {}
        for folded in (False,True):
            bundle = CompactMotionBundle.__new__(CompactMotionBundle)
            bundle._initialize(replay,generated,folded)
            runtimes['folded' if folded else 'unfused'] = bundle
        parity = {}
        for name,part in parts.items():
            checks = {mode:{'max_abs_logit_error':0.,'identical_predictions':True} for mode in runtimes}
            for offset in range(0,len(part['x']),32):
                if time.monotonic()-start > max_seconds:
                    raise TimeoutError('Compact export budget exceeded')
                x = part['x'][offset:offset+32]
                expected = baseline.predict(x)
                for mode,bundle in runtimes.items():
                    actual = bundle.predict(x)
                    error = float(np.max(np.abs(actual-expected)))
                    checks[mode]['max_abs_logit_error'] = max(checks[mode]['max_abs_logit_error'],error)
                    checks[mode]['identical_predictions'] &= bool(np.array_equal(actual.argmax(1),expected.argmax(1)))
                    if (mode == 'unfused' and not np.array_equal(actual,expected)) or error > 1e-10 or not checks[mode]['identical_predictions']:
                        raise ValueError('Compact runtime parity gate failed')
            parity[name] = dict(samples=len(part['x']),runtimes=checks)
        dev = parts['validation']['x']
        selected = runtimes['folded']
        for i in range(20):
            x = dev[i%len(dev):i%len(dev)+1]
            selected.predict(x); baseline.predict(x)
        timings = {'candidate':[],'incumbent':[]}
        for i in range(1000):
            if time.monotonic()-start > max_seconds:
                raise TimeoutError('Compact export budget exceeded')
            x = dev[i%len(dev):i%len(dev)+1]
            calls = [('candidate',selected.predict),('incumbent',baseline.predict)]
            if i%2: calls.reverse()
            for name,fn in calls:
                before = time.perf_counter_ns(); fn(x)
                timings[name].append((time.perf_counter_ns()-before)/1e6)
        latency = {k:{'p50_ms':float(np.percentile(v,50)),'p95_ms':float(np.percentile(v,95))} for k,v in timings.items()}
        size = (root/'model.safetensors').stat().st_size
        old_size = (Path(baseline_directory)/'model.safetensors').stat().st_size
        checks = {'model_below_10percent':size<.1*old_size,
                  'parity_all_admitted':True,
                  'p95_within_5percent':latency['candidate']['p95_ms']<=1.05*latency['incumbent']['p95_ms']}
        metadata = {'status':'complete','promotionAuthorized':False,'spec':SPEC,
                    'model_sha256':digest(root/'model.safetensors'),
                    'random_map_sha256':{k:tensor_hash(v) for k,v in generated.items()},
                    'baseline_model_sha256':prior['model_sha256'],
                    'baseline_receipt_sha256':hashlib.sha256(prior_bytes).hexdigest(),
                    'provenance':{'manifest_sha256':provenance['manifest_sha256']}}
        report = dict(metadata, provenance=provenance, evidence='SYNTHETIC' if provenance.get('evidence')=='SYNTHETIC' else 'MEASURED',
                      parity=parity,latency=latency,model_bytes=size,baseline_model_bytes=old_size,
                      stored_scalars=sum(v.size for v in compact.values()),runtime_mode='folded_float64',
                      retention_checks=checks,retained_as_research_candidate=all(checks.values()),
                      measured_10percent_speed_gain=all(latency['candidate'][q]<=.9*latency['incumbent'][q] for q in ('p50_ms','p95_ms')),
                      numpy_version=np.__version__,elapsed_seconds=time.monotonic()-start,
                      source_sha256={n:digest(Path(__file__).with_name(n)) for n in
                          ('espfi_motion_compact.py','espfi_motion.py','espfi_kernel.py','espfi_train.py','espfi.py')},
                      scope='Seed storage and float64 algebra only; unchanged features, no training or accuracy search. '
                            'Train/development parity only, no final test. 1000 rotating alternating batch1 calls include features/classifier/checks, exclude pooling/acquisition/IO. '
                            'Storage reduction is not a speed claim; promotion remains unauthorized.')
        if time.monotonic()-start > max_seconds:
            raise TimeoutError('Compact export budget exceeded')
        write_json(root/'metadata.json',metadata)
        write_json(root/'metrics.json',report)
        write_json(root/'status.json',{'status':'complete','promotionAuthorized':False})
        return report
    except Exception:
        write_json(root/'status.json',{'status':'failed','promotionAuthorized':False})
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('manifest','baseline','output'):
        parser.add_argument('--'+name,required=True)
    parser.add_argument('--max-seconds',type=int,default=600)
    args=parser.parse_args()
    from .espfi import load_development
    parts,provenance=load_development(args.manifest)
    print(json.dumps(export(parts,provenance,args.baseline,args.output,args.max_seconds),indent=2))

if __name__=='__main__':
    main()
