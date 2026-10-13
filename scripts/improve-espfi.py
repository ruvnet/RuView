#!/usr/bin/env python3
"""Bounded development-only generalization experiment."""
import os
# Set before importing NumPy/PyTorch for reproducible bounded CPU scheduling.
os.environ['OPENBLAS_NUM_THREADS'] = '1'
os.environ['OMP_NUM_THREADS'] = '1'
os.environ['MKL_NUM_THREADS'] = '1'
from csi_training.espfi_generalize import main

if __name__ == '__main__':
    main()
