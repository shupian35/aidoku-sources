#!/bin/bash
# Build all Aidoku sources using aidoku-cli
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PACKAGES=()

echo "=== Building Aidoku Sources ==="

# Build each source
for src_dir in "$SCRIPT_DIR"/sources/*/; do
    if [ -f "$src_dir/Cargo.toml" ]; then
        src_name=$(basename "$src_dir")
        echo ""
        echo "Building $src_name..."
        
        cd "$src_dir"
        
        # Use aidoku package
        aidoku package
        
        if [ -f "package.aix" ]; then
            PACKAGES+=("$src_dir/package.aix")
            echo "Built $src_name successfully"
        else
            echo "Failed to build $src_name"
        fi
        
        cd "$SCRIPT_DIR"
    fi
done

# Build source list
if [ ${#PACKAGES[@]} -gt 0 ]; then
    echo ""
    echo "=== Building Source List ==="
    aidoku build -o "$SCRIPT_DIR/public" -n "Development Source List" "${PACKAGES[@]}"
    
    echo ""
    echo "Build complete! ${#PACKAGES[@]} source(s) built."
else
    echo ""
    echo "No sources were built."
    exit 1
fi