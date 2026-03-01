#!/bin/bash

# ruvector CLI Demo
# This demonstrates the CLI functionality with a simple example

echo "🚀 ruvector CLI Demo"
echo "===================="
echo ""

# 1. Show version info
echo "1. Checking ruvector info..."
ruvector info
echo ""

# 2. Create a database
echo "2. Creating a new database..."
ruvector create demo.vec --dimension 3 --metric cosine
echo ""

# 3. Create sample data
echo "3. Creating sample vectors..."
cat > demo-vectors.json << 'EOF'
[
  {
    "id": "cat",
    "vector": [0.9, 0.1, 0.1],
    "metadata": {"animal": "cat", "category": "feline"}
  },
  {
    "id": "dog",
    "vector": [0.1, 0.9, 0.1],
    "metadata": {"animal": "dog", "category": "canine"}
  },
  {
    "id": "tiger",
    "vector": [0.8, 0.2, 0.15],
    "metadata": {"animal": "tiger", "category": "feline"}
  },
  {
    "id": "wolf",
    "vector": [0.2, 0.8, 0.15],
    "metadata": {"animal": "wolf", "category": "canine"}
  },
  {
    "id": "lion",
    "vector": [0.85, 0.15, 0.1],
    "metadata": {"animal": "lion", "category": "feline"}
  }
]
EOF
echo "   Created demo-vectors.json with 5 animals"
echo ""

# 4. Insert vectors
echo "4. Inserting vectors into database..."
ruvector insert demo.vec demo-vectors.json
echo ""

# 5. Show statistics
echo "5. Database statistics..."
ruvector stats demo.vec
echo ""

# 6. Search for cat-like animals
echo "6. Searching for cat-like animals (vector: [0.9, 0.1, 0.1])..."
ruvector search demo.vec --vector "[0.9, 0.1, 0.1]" --top-k 3
echo ""

# 7. Search for dog-like animals
echo "7. Searching for dog-like animals (vector: [0.1, 0.9, 0.1])..."
ruvector search demo.vec --vector "[0.1, 0.9, 0.1]" --top-k 3
echo ""

# 8. Run benchmark
echo "8. Running performance benchmark..."
ruvector benchmark --dimension 128 --num-vectors 1000 --num-queries 100
echo ""

# Cleanup
echo "9. Cleanup (removing demo files)..."
rm -f demo.vec demo-vectors.json
echo "   ✓ Demo files removed"
echo ""

echo "✅ Demo complete!"
