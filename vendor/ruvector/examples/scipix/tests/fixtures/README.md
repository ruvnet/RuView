# Test Fixtures for ruvector-scipix

This directory contains test fixtures including sample images, expected outputs, and configuration files for unit and integration tests.

## Directory Structure

```
fixtures/
├── images/           # Test images
│   ├── simple/      # Simple equations
│   ├── complex/     # Complex expressions
│   ├── matrices/    # Matrix expressions
│   └── symbols/     # Special mathematical symbols
├── expected/        # Expected LaTeX outputs
├── configs/         # Test configuration files
└── README.md        # This file
```

## Test Images

### Simple Equations
- `simple_addition.png` - Basic x + y
- `simple_fraction.png` - Simple fraction 1/2
- `quadratic.png` - Quadratic formula

### Complex Expressions
- `nested_fraction.png` - Nested fractions
- `integral.png` - Integral with limits
- `summation.png` - Summation notation

### Matrices
- `matrix_2x2.png` - 2x2 matrix
- `matrix_3x3.png` - 3x3 matrix

### Special Symbols
- `greek_letters.png` - Greek letters
- `operators.png` - Mathematical operators

## Expected Outputs

Each test image has a corresponding `.txt` file in the `expected/` directory containing the expected LaTeX output.

## Adding New Fixtures

1. Add the test image to the appropriate subdirectory
2. Create a corresponding expected output file
3. Update test cases in the unit tests to reference the new fixture

## Generating Test Images

You can use the synthetic data generator in `tests/testdata/synthetic_generator.rs` to create test images programmatically.

## Notes

- All test images should be in PNG format
- Expected outputs should use standard LaTeX notation
- Keep image sizes reasonable (< 1MB) for fast test execution
