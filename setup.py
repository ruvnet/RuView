"""
Setup script for WiFi-DensePose API
This file is maintained for backward compatibility.
The main configuration is in pyproject.toml.
"""

from setuptools import setup, find_packages
import os
import sys
from pathlib import Path

# Ensure we're in the right directory
if __name__ == "__main__":
    here = Path(__file__).parent.absolute()
    os.chdir(here)

# Read version from src/__init__.py
def get_version():
    """Get version from src/__init__.py"""
    version_file = here / "src" / "__init__.py"
    if version_file.exists():
        with open(version_file, 'r') as f:
            for line in f:
                if line.startswith('__version__'):
                    return line.split('=')[1].strip().strip('"').strip("'")
    return "1.0.0"

# Read long description from README
def get_long_description():
    """Get long description from README.md"""
    readme_file = here / "README.md"
    if readme_file.exists():
        with open(readme_file, 'r', encoding='utf-8') as f:
            return f.read()
    return "WiFi-based human pose estimation using CSI data and DensePose neural networks"

# Read requirements from requirements.txt if it exists
def get_requirements():
    """Get requirements from requirements.txt or use defaults"""
    requirements_file = here / "requirements.txt"
    if requirements_file.exists():
        with open(requirements_file, 'r') as f:
            return [line.strip() for line in f if line.strip() and not line.startswith('#')]
    
    # Default requirements (should match pyproject.toml)
    return [
        "fastapi>=0.104.0",
        "uvicorn[standard]>=0.24.0",
        "pydantic>=2.5.0",
        "pydantic-settings>=2.1.0",
        "sqlalchemy>=2.0.0",
        "alembic>=1.13.0",
        "asyncpg>=0.29.0",
        "psycopg2-binary>=2.9.0",
        "redis>=5.0.0",
        "aioredis>=2.0.0",
        "torch>=2.1.0",
        "torchvision>=0.16.0",
        "numpy>=1.24.0",
        "opencv-python>=4.8.0",
        "pillow>=10.0.0",
        "scikit-learn>=1.3.0",
        "scipy>=1.11.0",
        "matplotlib>=3.7.0",
        "pandas>=2.1.0",
        "scapy>=2.5.0",
        "pyserial>=3.5",
        "paramiko>=3.3.0",
        "click>=8.1.0",
        "rich>=13.6.0",
        "typer>=0.9.0",
        "python-multipart>=0.0.6",
        "python-jose[cryptography]>=3.3.0",
        "passlib[bcrypt]>=1.7.4",
        "python-dotenv>=1.0.0",
        "pyyaml>=6.0",
        "toml>=0.10.2",
        "prometheus-client>=0.19.0",
        "structlog>=23.2.0",
        "psutil>=5.9.0",
        "httpx>=0.25.0",
        "aiofiles>=23.2.0",
        "marshmallow>=3.20.0",
        "jsonschema>=4.19.0",
        "celery>=5.3.0",
        "kombu>=5.3.0",
    ]

# Development requirements
def get_dev_requirements():
    """Get development requirements"""
    return [
        "pytest>=7.4.0",
        "pytest-asyncio>=0.21.0",
        "pytest-cov>=4.1.0",
        "pytest-mock>=3.12.0",
        "pytest-xdist>=3.3.0",
        "black>=23.9.0",
        "isort>=5.12.0",
        "flake8>=6.1.0",
        "mypy>=1.6.0",
        "pre-commit>=3.5.0",
        "bandit>=1.7.0",
        "safety>=2.3.0",
    ]

# Check Python version
if sys.version_info < (3, 9):
    sys.exit("Python 3.9 or higher is required")

# Setup configuration
setup(
    name="wifi-densepose",
    version=get_version(),
    description="WiFi-based human pose estimation using CSI data and DensePose neural networks",
    long_description=get_long_description(),
    long_description_content_type="text/markdown",
    
    # Author information
    author="rUv",
    author_email="ruv@ruv.net",
    maintainer="rUv",
    maintainer_email="ruv@ruv.net",
    
    # URLs
    url="https://github.com/ruvnet/wifi-densepose",
    project_urls={
        "Documentation": "https://github.com/ruvnet/wifi-densepose#readme",
        "Source": "https://github.com/ruvnet/wifi-densepose",
        "Tracker": "https://github.com/ruvnet/wifi-densepose/issues",
    },
    
    # Package configuration
    packages=find_packages(include=["src", "src.*"]),
    package_dir={"": "."},
    
    # Include package data
    package_data={
        "src": [
            "*.yaml", "*.yml", "*.json", "*.toml", "*.cfg", "*.ini"
        ],
        "src.models": ["*.pth", "*.onnx", "*.pt"],
        "src.config": ["*.yaml", "*.yml", "*.json"],
    },
    include_package_data=True,
    
    # Requirements
    python_requires=">=3.9",
    install_requires=get_requirements(),
    extras_require={
        "dev": get_dev_requirements(),
        "docs": [
            "sphinx>=7.2.0",
            "sphinx-rtd-theme>=1.3.0",
            "sphinx-autodoc-typehints>=1.25.0",
            "myst-parser>=2.0.0",
        ],
        "gpu": [
            "torch>=2.1.0",
            "torchvision>=0.16.0",
            "nvidia-ml-py>=12.535.0",
        ],
        "monitoring": [
            "grafana-api>=1.0.3",
            "influxdb-client>=1.38.0",
            "elasticsearch>=8.10.0",
        ],
        "deployment": [
            "gunicorn>=21.2.0",
            "docker>=6.1.0",
            "kubernetes>=28.1.0",
        ],
    },
    
    # Entry points
    entry_points={
        "console_scripts": [
            "wifi-densepose=src.cli:cli",
            "wdp=src.cli:cli",
        ],
        "wifi_densepose.plugins": [
            # Plugin entry points for extensibility
        ],
    },
    
    # Classification
    classifiers=[
        "Development Status :: 4 - Beta",
        "Intended Audience :: Developers",
        "Intended Audience :: Science/Research",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Topic :: Scientific/Engineering :: Artificial Intelligence",
        "Topic :: Scientific/Engineering :: Image Processing",
        "Topic :: System :: Networking",
        "Topic :: Software Development :: Libraries :: Python Modules",
    ],
    
    # Keywords
    keywords=[
        "wifi", "csi", "pose-estimation", "densepose", "neural-networks",
        "computer-vision", "machine-learning", "iot", "wireless-sensing"
    ],
    
    # License
    license="MIT",
    
    # Zip safe
    zip_safe=False,
    
    # Platform
    platforms=["any"],
)