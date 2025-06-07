# Install PyTorch and other dependencies
import subprocess
import sys

def install_package(package):
    subprocess.check_call([sys.executable, "-m", "pip", "install", package])

try:
    import torch
    print("PyTorch already installed")
except ImportError:
    print("Installing PyTorch...")
    install_package("torch")
    install_package("torchvision")

try:
    import numpy
    print("NumPy already installed")
except ImportError:
    print("Installing NumPy...")
    install_package("numpy")

print("All packages ready!")