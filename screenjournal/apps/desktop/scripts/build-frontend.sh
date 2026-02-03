#!/bin/bash
# Frontend Build Script for Tauri
# This script ensures TypeScript compilation succeeds before building Next.js
# and provides proper error handling for Tauri's beforeBuildCommand
#
# This script is run from the src-tauri directory context by Tauri,
# so we need to change to the parent directory first.

set -e  # Exit on any error

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
# Change to the desktop app directory (parent of scripts)
APP_DIR="$( cd "$SCRIPT_DIR/.." && pwd )"
cd "$APP_DIR"

echo "🔍 Running TypeScript type check..."
npm run check-types

if [ $? -ne 0 ]; then
    echo "❌ TypeScript type check failed"
    exit 1
fi

echo "✅ TypeScript type check passed"
echo "🔨 Building Next.js frontend..."
npm run build

if [ $? -ne 0 ]; then
    echo "❌ Next.js build failed"
    exit 1
fi

echo "✅ Frontend build completed successfully"

# Verify output directory exists (relative to app directory)
if [ ! -d "out" ]; then
    echo "❌ Output directory 'out' not found after build"
    exit 1
fi

# Verify index.html exists
if [ ! -f "out/index.html" ]; then
    echo "❌ index.html not found in output directory"
    exit 1
fi

echo "✅ Output validation passed"
exit 0

