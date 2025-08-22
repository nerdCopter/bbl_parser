#!/bin/bash
# Developer setup script for bbl_parser project
# This script sets up the development environment with proper formatting checks

set -e

echo "🚀 Setting up bbl_parser development environment..."

# Check if we're in a git repository
if [ ! -d ".git" ]; then
    echo "❌ Error: This script must be run from the root of the git repository."
    exit 1
fi

# Install pre-commit hook
echo "📥 Installing pre-commit hook for automatic formatting..."
if [ -f ".github/pre-commit-hook.sh" ]; then
    cp .github/pre-commit-hook.sh .git/hooks/pre-commit
    chmod +x .git/hooks/pre-commit
    echo "✅ Pre-commit hook installed successfully."
else
    echo "⚠️  Pre-commit hook not found. Skipping..."
fi

# Run initial formatting check
echo "🔧 Running initial formatting check..."
if ! cargo fmt --all -- --check; then
    echo "⚠️  Code needs formatting. Applying automatic formatting..."
    cargo fmt --all
    echo "✅ Code has been formatted."
else
    echo "✅ Code formatting is already correct."
fi

# Run clippy check
echo "🔍 Running clippy check..."
if ! cargo clippy --all-targets --all-features -- -D warnings; then
    echo "❌ Clippy found issues. Please fix them before continuing."
    exit 1
else
    echo "✅ Clippy check passed."
fi

# Run tests
echo "🧪 Running tests..."
if ! cargo test --verbose; then
    echo "❌ Tests failed. Please fix them before continuing."
    exit 1
else
    echo "✅ All tests passed."
fi

echo ""
echo "🎉 Development environment setup complete!"
echo ""
echo "📋 Remember to always run these commands before committing:"
echo "   cargo fmt --all                                     # Format code"
echo "   cargo clippy --all-targets --all-features -- -D warnings  # Check for issues"
echo "   cargo test --verbose                                # Run tests"
echo "   cargo test --features=cli --verbose                 # Run CLI tests"
echo "   cargo build --release                               # Build release"
echo ""
echo "🔧 The pre-commit hook will automatically format your code on each commit."
