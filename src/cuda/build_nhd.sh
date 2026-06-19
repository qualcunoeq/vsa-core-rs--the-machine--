#!/usr/bin/env bash
# Build the CUDA NHD kernel shared library.
# Run this from the crate root directory.
set -euo pipefail

# Locate CUDA toolkit
CUDA_PATH="${CUDA_PATH:-}"
if [ -z "$CUDA_PATH" ]; then
    # Try pip installation
    PIP_CUDA="$HOME/odysseus/data/local/lib/python3.12/site-packages/nvidia/cu13"
    if [ -f "$PIP_CUDA/bin/nvcc" ]; then
        CUDA_PATH="$PIP_CUDA"
    elif [ -d /usr/local/cuda ]; then
        # Find the newest version
        CUDA_PATH=$(ls -d /usr/local/cuda* 2>/dev/null | sort -V | tail -1)
    else
        echo "ERROR: CUDA toolkit not found. Set CUDA_PATH or install nvidia-cuda-runtime."
        exit 1
    fi
fi

echo "Using CUDA toolkit: $CUDA_PATH"

export PATH="$CUDA_PATH/bin:$PATH"
export LD_LIBRARY_PATH="$CUDA_PATH/lib:${LD_LIBRARY_PATH:-}"

# Determine architecture: RTX 5070 Ti = sm_120 (Blackwell)
ARCH="${ARCH:-sm_120}"

echo "Compiling for arch=$ARCH..."

nvcc -shared -o src/cuda/libnhd.so \
    -arch="$ARCH" \
    -Xcompiler -fPIC \
    -I "$CUDA_PATH/include" \
    src/cuda/nhd_wrapper.cu \
    -L "$CUDA_PATH/lib" \
    -lcudart

echo "Done: src/cuda/libnhd.so ($(wc -c < src/cuda/libnhd.so) bytes)"
