#!/usr/bin/env python3
"""Bounded nonlinear development experiment; never reads final-test arrays."""
import os
os.environ['OPENBLAS_NUM_THREADS'] = '1'
os.environ['OMP_NUM_THREADS'] = '1'
os.environ['MKL_NUM_THREADS'] = '1'
from csi_training.espfi_kernel import main

if __name__ == '__main__':
    main()
