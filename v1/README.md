# WiFi-DensePose v1 (Python Implementation)

This directory contains the original Python implementation of WiFi-DensePose.

## Structure

```
v1/
├── src/                    # Python source code
│   ├── api/               # REST API endpoints
│   ├── config/            # Configuration management
│   ├── core/              # Core processing logic
│   ├── database/          # Database models and migrations
│   ├── hardware/          # Hardware interfaces
│   ├── middleware/        # API middleware
│   ├── models/            # Neural network models
│   ├── services/          # Business logic services
│   └── tasks/             # Background tasks
├── tests/                  # Test suite
├── docs/                   # Documentation
├── scripts/               # Utility scripts
├── data/                  # Data files
├── setup.py               # Package setup
├── test_application.py    # Application tests
└── test_auth_rate_limit.py # Auth/rate limit tests
```

## Requirements

- Python 3.10+
- PyTorch 2.0+
- FastAPI
- PostgreSQL/SQLite

## Installation

```bash
cd v1
pip install -e .
```

## Usage

```bash
# Start API server
python -m src.main

# Run tests
pytest tests/
```

## Note

This is the legacy Python implementation. For the new Rust implementation with improved performance, see `/rust-port/wifi-densepose-rs/`.
