#!/bin/bash
# Pre-commit hook to automatically format Rust code
# Install this by copying to .git/hooks/pre-commit and making it executable

echo "Running pre-commit formatting check..."

# Check if cargo fmt is available
if ! command -v cargo &> /dev/null; then
    echo "❌ cargo not found. Please install Rust toolchain."
    exit 1
fi

# Format the code
echo "🔧 Auto-formatting Rust code..."
cargo fmt --all

# Check if there are any changes after formatting
if ! git diff --exit-code --quiet; then
    echo "✅ Code has been automatically formatted."
    echo "📝 The following files were formatted:"
    git diff --name-only
    echo ""
    echo "🔄 Please review the changes and commit again."
    echo "   The formatting has been applied automatically."
    exit 1
else
    echo "✅ Code formatting is already correct."
fi

echo "✅ Pre-commit checks passed!"
