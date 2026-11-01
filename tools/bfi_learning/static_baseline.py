#!/usr/bin/env python3
"""Test how much custom-split identity information survives temporal averaging.

This train/validation control uses one mean amplitude vector per recording.
It does not score the held-out test partition or establish a physical cause.
"""
import argparse
import hashlib
import json
import sys
from pathlib import Path
import numpy as np
import train as trainer_contract
from train import load_dataset, build_splits, outside_git, require, write_json


def score_static(dataset):
    # Reuse source provenance and partition checks from the identity trainer.
    splits=build_splits(dataset,32,32,'identity',identity_protocol='published_files',stride=8)
    classes=splits['classes']; frames=dataset['frames']
    sets={}
    for partition in ('train','validation'):
        records=[s for s in dataset['metadata']['sessions'] if s['split']==partition]
        x=np.stack([frames[s['start']:s['end']].mean(axis=0,dtype=np.float64) for s in records])
        y=np.array([classes.index(s['identity']) for s in records])
        sets[partition]=(x,y)
    x,y=sets['train'];v,truth=sets['validation']
    center=x.mean(axis=0);scale=np.maximum(x.std(axis=0),1e-5)
    x=(x-center)/scale;v=(v-center)/scale
    require(np.isfinite(x).all() and np.isfinite(v).all(),'nonfinite standardized means')
    centroids=np.stack([x[y==label].mean(axis=0) for label in range(len(classes))])
    nearest=np.argmin(np.square(v[:,None,:]-centroids[None,:,:]).sum(axis=2),axis=1)
    # Fixed alpha=1; no search against validation or test outcomes.
    design=np.column_stack([np.ones(len(x)),x]);query=np.column_stack([np.ones(len(v)),v])
    penalty=np.eye(design.shape[1]);penalty[0,0]=0
    weights=np.linalg.solve(design.T@design+penalty,design.T@np.eye(len(classes))[y])
    ridge=np.argmax(query@weights,axis=1)
    majority=int(np.bincount(y).argmax())
    results={}
    for name,prediction in [('nearest_centroid',nearest),('ridge_alpha_1',ridge)]:
        confusion=np.zeros((len(classes),len(classes)),dtype=np.int64)
        np.add.at(confusion,(truth,prediction),1)
        results[name]={'recording_accuracy':float(np.mean(prediction==truth)),
                       'balanced_accuracy':float(np.mean(np.diag(confusion)/confusion.sum(axis=1))),
                       'confusion_matrix':confusion.tolist()}
    return {'signal':'public_CSI_amplitude','split_protocol':'archive_directory_custom',
            'representation':'mean of all samples within each source recording; no temporal order',
            'normalization_fit':'training_recording_means_only','ridge_alpha':1,
            'train_recordings':len(x),'validation_recordings':len(v),
            'majority_baseline_accuracy':float(np.mean(truth==majority)),
            'results':results,'test':None,'test_used_for_selection':False,
            'interpretation':'High accuracy means temporal order is unnecessary for this split. It does not identify whether body, room, position, device or acquisition conditions supplied the signal.',
            'acquisition_session_disjointness':'unknown'}


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('dataset','metadata','output'):p.add_argument('--'+name,required=True)
    args=p.parse_args()
    try:
        output=outside_git(args.output)
        require(not output.exists(),'output must be a new private directory')
        dataset=load_dataset(args.dataset,args.metadata)
        require(dataset['metadata'].get('source_kind')=='ntu_fi_humanid','control requires public NTU-Fi data')
        result=score_static(dataset)
        result.update(status='COMPLETED',evidence='MEASURED',
                      dataset_sha256=dataset['metadata']['dataset_sha256'],
                      metadata_sha256=dataset['metadata_sha256'],
                      source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                      trainer_contract_sha256=hashlib.sha256(Path(trainer_contract.__file__).read_bytes()).hexdigest(),
                      python_version=sys.version.split()[0],numpy_version=np.__version__)
        output.mkdir(parents=True,mode=0o700)
        write_json(output/'metrics.json',result)
        print(json.dumps({k:v for k,v in result.items() if k!='results'}))
        return 0
    except (ValueError,OSError,np.linalg.LinAlgError) as error:
        p.exit(1,str(error)+'\n')


if __name__=='__main__':raise SystemExit(main())
