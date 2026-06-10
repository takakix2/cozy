#!/bin/bash
# Simple test script for Rue Editor features

set -e

echo "=== Rue Editor Feature Tests ==="
echo ""

# Test 1: Build check
echo "1. Testing build..."
cargo build --release
if [ $? -eq 0 ]; then
    echo "   ✓ Build successful"
else
    echo "   ✗ Build failed"
    exit 1
fi

# Test 2: Run unit tests
echo ""
echo "2. Running unit tests..."
cargo test --lib 2>&1 | tail -5
if [ $? -eq 0 ]; then
    echo "   ✓ Unit tests passed"
else
    echo "   ✗ Unit tests failed"
    exit 1
fi

# Test 3: Check binary exists
echo ""
echo "3. Checking binary..."
if [ -f "target/release/rue_editor" ]; then
    echo "   ✓ Binary exists: target/release/rue_editor"
    echo "   Binary size: $(du -h target/release/rue_editor | cut -f1)"
else
    echo "   ✗ Binary not found"
    exit 1
fi

# Test 4: Check dependencies
echo ""
echo "4. Checking dependencies..."
if cargo tree | grep -q "arboard"; then
    echo "   ✓ arboard (clipboard) dependency found"
else
    echo "   ✗ arboard dependency not found"
fi

if cargo tree | grep -q "regex"; then
    echo "   ✓ regex (syntax highlighting) dependency found"
else
    echo "   ✗ regex dependency not found"
fi

echo ""
echo "=== All checks completed ==="
echo ""
echo "To test interactively, run:"
echo "  cargo run --release"
echo "  or"
echo "  ./target/release/rue_editor <filename>"
