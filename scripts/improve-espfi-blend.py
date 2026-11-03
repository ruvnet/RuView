#!/usr/bin/env python3
"""Run the frozen five-weight development blend with bounded CPU threads."""
import os
for name in ('OPENBLAS_NUM_THREADS', 'OMP_NUM_THREADS', 'MKL_NUM_THREADS'):
    os.environ[name] = '1'
from csi_training.espfi_blend import main
if __name__ == '__main__':
    main()
